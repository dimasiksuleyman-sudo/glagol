//! Glagol as a recognition server for other computers in the office (GL-010).
//!
//! An OpenAI-compatible HTTP endpoint (`POST /v1/audio/transcriptions`,
//! `GET /v1/models`) over this computer's Russian GigaAM engine, so other
//! Glagol copies in «organization server» mode — and other compatible
//! clients — dictate through it. Off by default; only private networks are
//! served; an access key is always required; requests queue on the engine.
//!
//! Privacy: speech and text are never logged; client addresses are kept only
//! in memory as a count of recently active computers.

pub mod audio;
pub mod auth;
pub mod awake;
pub mod http;
pub mod multipart;
pub mod net;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::StatusCode;
use hyper_util::rt::{TokioIo, TokioTimer};
use serde::Serialize;
use tokio::sync::{oneshot, Semaphore};

/// Default TCP port of the server.
pub const DEFAULT_PORT: u16 = 8765;
/// Open connections accepted at once; further ones are closed. Above the
/// recognition queue ([`http::MAX_PENDING`]) so a full queue answers 429
/// instead of resetting connections.
const MAX_CONNECTIONS: usize = 2 * http::MAX_PENDING;
/// A client counts as connected while it sent a request this recently.
const ACTIVE_WINDOW: Duration = Duration::from_secs(10 * 60);

/// What the server recognizes with.
pub trait Recognizer: Send + Sync + 'static {
    /// The model id served, or a message why recognition is unavailable.
    fn model(&self) -> Result<String, String>;
    /// Recognize 16 kHz mono samples. Blocking; called off the async runtime.
    fn recognize(&self, pcm: &[f32]) -> Result<String, String>;
}

/// The host's Russian GigaAM engine. Requests wait for the engine like the
/// host's own dictation does, on the device (processor or graphics card) the
/// host chose.
pub struct GigaAm {
    /// The local model store shared with dictation.
    pub models: Arc<crate::stt::local::LocalModels>,
    /// The host's currently selected Russian model id (read per request, so
    /// switching the model in Settings applies without a restart).
    pub model_source: Box<dyn Fn() -> String + Send + Sync>,
}

impl Recognizer for GigaAm {
    fn model(&self) -> Result<String, String> {
        let id = (self.model_source)();
        let installed = self
            .models
            .status()
            .models
            .iter()
            .any(|m| m.id == id && m.language == "ru" && m.installed);
        if installed {
            Ok(id)
        } else {
            Err("the Russian recognition model is not installed on the server".into())
        }
    }

    fn recognize(&self, pcm: &[f32]) -> Result<String, String> {
        let id = self.model()?;
        self.models.recognize(&id, pcm).map(|(text, ..)| text)
    }
}

/// Request counters; addresses stay in memory only.
#[derive(Default)]
pub struct Stats {
    requests: AtomicU64,
    failures: AtomicU64,
    recent: Mutex<HashMap<IpAddr, Instant>>,
}

impl Stats {
    fn record(&self, status: StatusCode) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        if !status.is_success() {
            self.failures.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn seen(&self, peer: IpAddr) {
        if let Ok(mut recent) = self.recent.lock() {
            let now = Instant::now();
            recent.retain(|_, at| now.duration_since(*at) < ACTIVE_WINDOW);
            recent.insert(peer, now);
        }
    }

    fn active_clients(&self) -> usize {
        self.recent.lock().map_or(0, |recent| {
            recent
                .values()
                .filter(|at| at.elapsed() < ACTIVE_WINDOW)
                .count()
        })
    }
}

/// Shared state of one running server.
pub struct Context {
    key: String,
    recognizer: Arc<dyn Recognizer>,
    stats: Stats,
    pending: AtomicUsize,
}

/// What the settings screen shows about a running server.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Status {
    /// The bound port.
    pub port: u16,
    /// Requests answered since start.
    pub requests: u64,
    /// Requests answered with an error status since start.
    pub failures: u64,
    /// Computers that sent a request in the last 10 minutes.
    pub active_clients: usize,
    /// Recognitions running or waiting right now.
    pub pending: usize,
}

/// A running server; dropping it (or [`Server::stop`]) stops accepting.
pub struct Server {
    addr: SocketAddr,
    ctx: Arc<Context>,
    shutdown: Option<oneshot::Sender<()>>,
}

impl Server {
    /// Bind `addr` and serve. Fails if the port is taken. Must be called
    /// inside a Tokio runtime.
    pub async fn start(
        addr: SocketAddr,
        key: String,
        recognizer: Arc<dyn Recognizer>,
    ) -> Result<Server, std::io::Error> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        let ctx = Arc::new(Context {
            key,
            recognizer,
            stats: Stats::default(),
            pending: AtomicUsize::new(0),
        });
        let (shutdown, mut stopped) = oneshot::channel::<()>();
        let connections = Arc::new(Semaphore::new(MAX_CONNECTIONS));
        let serving = ctx.clone();
        tokio::spawn(async move {
            loop {
                let accepted = tokio::select! {
                    _ = &mut stopped => break,
                    accepted = listener.accept() => accepted,
                };
                let Ok((stream, peer)) = accepted else {
                    continue;
                };
                if !net::is_allowed_peer(peer.ip()) {
                    continue; // dropping the stream closes it
                }
                let Ok(permit) = connections.clone().try_acquire_owned() else {
                    continue;
                };
                let ctx = serving.clone();
                tokio::spawn(async move {
                    let service = service_fn(move |req| http::handle(req, ctx.clone(), peer.ip()));
                    // One dictation is one request: closing after the answer
                    // keeps idle clients from holding connection slots.
                    let _ = http1::Builder::new()
                        .timer(TokioTimer::new())
                        .header_read_timeout(Duration::from_secs(10))
                        .keep_alive(false)
                        .serve_connection(TokioIo::new(stream), service)
                        .await;
                    drop(permit);
                });
            }
            tracing::info!("LAN recognition server stopped");
        });
        tracing::info!(port = addr.port(), "LAN recognition server started");
        Ok(Server {
            addr,
            ctx,
            shutdown: Some(shutdown),
        })
    }

    /// The bound address (the real port when started on port 0).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Current counters.
    pub fn status(&self) -> Status {
        Status {
            port: self.addr.port(),
            requests: self.ctx.stats.requests.load(Ordering::Relaxed),
            failures: self.ctx.stats.failures.load(Ordering::Relaxed),
            active_clients: self.ctx.stats.active_clients(),
            pending: self.ctx.pending.load(Ordering::SeqCst),
        }
    }

    /// Stop accepting connections; requests in flight finish.
    pub fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

/// The code a client pastes to connect: `glagol://host:port/?key=…`.
pub fn connection_code(host: &str, port: u16, key: &str) -> String {
    format!("glagol://{host}:{port}/?key={key}")
}

/// A parsed connection code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    /// `http://host:port/v1` for the «organization server» profile.
    pub base_url: String,
    /// The access key.
    pub key: String,
}

/// Parse a connection code. Only private-network addresses are accepted, the
/// same rule the client applies to plain-HTTP servers.
pub fn parse_connection_code(code: &str) -> Result<Connection, String> {
    let rest = code
        .trim()
        .strip_prefix("glagol://")
        .ok_or("Это не код подключения Глагола.")?;
    let (authority, query) = rest.split_once("/?").ok_or("В коде нет ключа доступа.")?;
    let key = query
        .split('&')
        .find_map(|p| p.strip_prefix("key="))
        .filter(|k| k.len() == 64 && k.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or("В коде неверный ключ доступа.")?;
    let addr: SocketAddr = authority
        .parse()
        .map_err(|_| "В коде неверный адрес сервера.")?;
    if !net::is_allowed_peer(addr.ip()) {
        return Err("Сервер Глагола должен быть в локальной сети.".into());
    }
    Ok(Connection {
        base_url: format!("http://{addr}/v1"),
        key: key.to_string(),
    })
}

#[cfg(test)]
mod tests;
