//! Request handling: an OpenAI-compatible subset that Glagol's own
//! «organization server» client (and other clients) already speak.

use std::convert::Infallible;
use std::net::IpAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, RETRY_AFTER};
use hyper::{Method, Request, Response, StatusCode};

use super::{audio, auth, multipart, Context};

/// Largest accepted request body (OpenAI's own transcription limit).
pub const MAX_BODY_BYTES: usize = 25 * 1024 * 1024;
/// Recognitions running or waiting at once; more are told to retry (429).
/// A GTX 1060 recognizes 6 s of speech in ~0.4 s (2026-11-19 benchmark), so
/// a full queue still answers within seconds; Glagol clients retry a 429.
pub const MAX_PENDING: usize = 32;
/// A recognition that takes longer than this answers 504.
const RECOGNITION_TIMEOUT: Duration = Duration::from_secs(120);

type Reply = Response<Full<Bytes>>;

fn reply(status: StatusCode, content_type: &str, body: impl Into<Bytes>) -> Reply {
    let mut response = Response::new(Full::new(body.into()));
    *response.status_mut() = status;
    response.headers_mut().insert(
        CONTENT_TYPE,
        content_type.parse().expect("static content type"),
    );
    response
}

fn json(status: StatusCode, value: serde_json::Value) -> Reply {
    reply(status, "application/json", value.to_string())
}

/// An OpenAI-style error body; `message` is safe to show (no user data).
fn error(status: StatusCode, kind: &str, message: &str) -> Reply {
    json(
        status,
        serde_json::json!({ "error": { "message": message, "type": kind } }),
    )
}

/// Decrements the pending counter however the request ends.
struct PendingGuard<'a>(&'a std::sync::atomic::AtomicUsize);
impl Drop for PendingGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Serve one request. Never fails at the transport level: every problem is an
/// HTTP status with a short English message.
pub async fn handle(
    req: Request<Incoming>,
    ctx: Arc<Context>,
    peer: IpAddr,
) -> Result<Reply, Infallible> {
    let started = Instant::now();
    let response = route(req, &ctx, peer).await;
    let status = response.status();
    ctx.stats.record(status);
    tracing::debug!(
        status = status.as_u16(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "LAN server request"
    );
    Ok(response)
}

async fn route(req: Request<Incoming>, ctx: &Context, peer: IpAddr) -> Reply {
    let path = req.uri().path().to_string();
    // Clients are given `http://host:port/v1`; accept the bare paths too.
    let endpoint = path.strip_prefix("/v1").unwrap_or(&path);
    match (req.method(), endpoint) {
        (&Method::GET, "/" | "") => reply(
            StatusCode::OK,
            "text/plain; charset=utf-8",
            "Glagol LAN recognition server\n",
        ),
        (&Method::GET, "/models") => {
            if !authorized(&req, ctx) {
                return unauthorized();
            }
            ctx.stats.seen(peer);
            match ctx.recognizer.model() {
                Ok(model) => json(
                    StatusCode::OK,
                    serde_json::json!({
                        "object": "list",
                        "data": [{ "id": model, "object": "model", "owned_by": "glagol" }]
                    }),
                ),
                Err(message) => error(StatusCode::SERVICE_UNAVAILABLE, "unavailable", &message),
            }
        }
        (&Method::POST, "/audio/transcriptions") => {
            if !authorized(&req, ctx) {
                return unauthorized();
            }
            ctx.stats.seen(peer);
            transcribe(req, ctx).await
        }
        (_, "/models" | "/audio/transcriptions") => error(
            StatusCode::METHOD_NOT_ALLOWED,
            "invalid_request_error",
            "method not allowed",
        ),
        _ => error(StatusCode::NOT_FOUND, "invalid_request_error", "not found"),
    }
}

fn authorized(req: &Request<Incoming>, ctx: &Context) -> bool {
    let header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    auth::authorized(header, &ctx.key)
}

fn unauthorized() -> Reply {
    error(
        StatusCode::UNAUTHORIZED,
        "authentication_error",
        "missing or wrong access key",
    )
}

async fn transcribe(req: Request<Incoming>, ctx: &Context) -> Reply {
    let bad = |message: &str| error(StatusCode::BAD_REQUEST, "invalid_request_error", message);
    let Some(boundary) = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(multipart::boundary)
    else {
        return bad("expected multipart/form-data");
    };
    let declared = req
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|n| n > MAX_BODY_BYTES) {
        return error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "invalid_request_error",
            "audio is larger than 25 MB",
        );
    }
    let body = match Limited::new(req.into_body(), MAX_BODY_BYTES)
        .collect()
        .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(_) => {
            return error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "invalid_request_error",
                "audio is larger than 25 MB or the upload broke off",
            )
        }
    };
    let form = match multipart::parse(&body, &boundary) {
        Ok(form) => form,
        Err(e) => return bad(e.0),
    };
    drop(body);
    match form.fields.get("language").map(|l| l.trim()) {
        None | Some("") | Some("ru") | Some("auto") => {}
        Some(_) => return bad("this Glagol server recognizes Russian only (language=ru)"),
    }
    let as_text = match form.fields.get("response_format").map(|f| f.trim()) {
        None | Some("") | Some("json") | Some("verbose_json") => false,
        Some("text") => true,
        Some(_) => return bad("response_format must be json or text"),
    };
    if let Err(message) = ctx.recognizer.model() {
        return error(StatusCode::SERVICE_UNAVAILABLE, "unavailable", &message);
    }
    if ctx.pending.fetch_add(1, Ordering::SeqCst) >= MAX_PENDING {
        ctx.pending.fetch_sub(1, Ordering::SeqCst);
        let mut busy = error(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limit_error",
            "server is busy, retry in a few seconds",
        );
        busy.headers_mut()
            .insert(RETRY_AFTER, "2".parse().expect("static header"));
        return busy;
    }
    let _pending = PendingGuard(&ctx.pending);
    let recognizer = ctx.recognizer.clone();
    let file = form.file;
    let started = Instant::now();
    let work = tokio::task::spawn_blocking(move || {
        let pcm = audio::decode_wav(&file)
            .map_err(|m| (StatusCode::UNSUPPORTED_MEDIA_TYPE, m.to_string()))?;
        let audio_ms = (pcm.len() / 16) as u64;
        recognizer
            .recognize(&pcm)
            .map(|text| (text, audio_ms))
            .map_err(|m| (StatusCode::INTERNAL_SERVER_ERROR, m))
    });
    let outcome = match tokio::time::timeout(RECOGNITION_TIMEOUT, work).await {
        Err(_) => {
            return error(
                StatusCode::GATEWAY_TIMEOUT,
                "server_error",
                "recognition timed out",
            )
        }
        Ok(Err(_)) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "server_error",
                "recognition failed",
            )
        }
        Ok(Ok(outcome)) => outcome,
    };
    match outcome {
        Ok((text, audio_ms)) => {
            tracing::info!(
                audio_ms,
                elapsed_ms = started.elapsed().as_millis() as u64,
                "LAN server recognition"
            );
            if as_text {
                reply(StatusCode::OK, "text/plain; charset=utf-8", text)
            } else {
                json(StatusCode::OK, serde_json::json!({ "text": text }))
            }
        }
        Err((status, message)) => {
            let kind = if status.is_client_error() {
                "invalid_request_error"
            } else {
                "server_error"
            };
            error(status, kind, &message)
        }
    }
}
