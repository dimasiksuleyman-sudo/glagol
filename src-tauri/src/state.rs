//! Shared application state managed by Tauri.
//!
//! [`AppState`] holds:
//!
//! - A pre-built [`reqwest::Client`] with TLS pinning to the embedded
//!   НУЦ Минцифры root certificate — created once at startup so every
//!   command reuses the same connection pool.
//! - A lazily-initialised [`SaluteAuth`] wrapped in
//!   `Mutex<Option<Arc<...>>>`. The `Option` lets us start the app
//!   without credentials configured; the `Arc` lets multiple commands
//!   share the auth instance (and its token cache) while the `Mutex`
//!   keeps mutations sequenced. Holding the lock is only required to
//!   read or replace the `Option` — never across the network calls
//!   that use the resulting `Arc<SaluteAuth>`.

use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::dictation::{DictationPhase, RecorderHandle};
use crate::salute::auth::SaluteAuth;

/// Process-wide Tauri state.
///
/// Registered via [`tauri::Builder::manage`] in `lib.rs::run` and
/// retrieved by commands through `tauri::State<'_, AppState>`.
pub struct AppState {
    /// Shared HTTP client (TLS-pinned). Cheap to clone — internally
    /// `Arc`'d by reqwest.
    pub http_client: reqwest::Client,

    /// Cached [`SaluteAuth`] for the currently configured Authorization
    /// Key. `None` when no credentials have been saved or after
    /// `delete_credentials` / `set_credentials` resets the state.
    ///
    /// `tokio::sync::Mutex` (not `std::sync::Mutex`) because commands
    /// are `async` and may await with the guard held briefly during
    /// get-or-insert.
    pub salute_auth: tokio::sync::Mutex<Option<Arc<SaluteAuth>>>,

    /// Application database connection.
    ///
    /// `std::sync::Mutex` (not `tokio::sync::Mutex`) because rusqlite is
    /// synchronous — every DB call would have to `.await` the lock for no
    /// benefit. Single-user app, sub-millisecond operations, no contention.
    /// Eagerly initialised by the Tauri setup hook (see `lib.rs::run`);
    /// migrations must succeed at app start or the app refuses to launch.
    pub db: Mutex<Connection>,

    /// Process-lifetime "the STT key checked out against the provider" flag
    /// (Sprint 6 PR1). Mirrors the cache-first contract of
    /// [`salute_auth`](Self::salute_auth): the mount-time probe
    /// (`test_stt_key` with `force = false`) trusts a prior success and skips
    /// the network, while the user-initiated Test button (`force = true`)
    /// always revalidates. Reset to `false` whenever the key is set or
    /// deleted. `tokio::sync::Mutex` because dictation commands are `async`.
    pub stt_key_validated: tokio::sync::Mutex<bool>,

    /// Handle to the dedicated microphone recorder thread (Sprint 6 PR2). Just
    /// the send side of the recorder's command queue — cheap to clone. The
    /// thread itself is spawned in `lib.rs::run`'s setup hook and owns the
    /// non-`Send` `cpal::Stream`.
    pub recorder: RecorderHandle,

    /// App-level dictation phase (D13). `std::sync::Mutex` and **never** held
    /// across an `.await` (project convention); the guard is block-scoped
    /// around each transition. PR2 only establishes the field — the state
    /// machine is driven by the pipeline in PR3.
    pub dictation: Mutex<DictationPhase>,

    /// The release signal for the in-flight dictation (Sprint 6 PR3). The
    /// hotkey `Pressed` handler stores a `oneshot::Sender` here and spawns the
    /// pipeline with the matching `Receiver`; the `Released` handler `take()`s
    /// and fires it, ending the recording. `take()` semantics make a second
    /// `Released` (or a lost `Pressed`/`Released` ordering) a no-op. Block-scoped
    /// `std::sync::Mutex`, never held across an `.await`.
    pub dictation_stop: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,

    /// Monotonic dictation-session counter (Sprint 6 PR3). Each `Pressed`
    /// increments it and captures the new value as its session token; a
    /// session's teardown only resets the shared tray/phase/sender state if the
    /// counter still matches its token. This defuses the composition mine where
    /// a fresh `Pressed` lands in the microsecond gap after the pipeline set
    /// `Idle` but before the previous session's cleanup ran — without the guard,
    /// the stale cleanup would clobber the new session (D10-class hazard).
    pub dictation_generation: std::sync::atomic::AtomicU64,

    /// The tracing-appender flush guard (Sprint 6 PR3.1). In release builds the
    /// non-blocking rolling-file writer keeps flushing only while its
    /// `WorkerGuard` is alive; dropping it silently stops logging (D-L4). It is
    /// parked here for the process lifetime, set once from the setup hook via
    /// [`AppState::set_log_guard`]. `None` in dev (stdout needs no guard).
    pub log_guard: Mutex<Option<tracing_appender::non_blocking::WorkerGuard>>,
}

impl AppState {
    /// Construct fresh state with no auth configured.
    pub fn new(http_client: reqwest::Client, db: Connection, recorder: RecorderHandle) -> Self {
        Self {
            http_client,
            salute_auth: tokio::sync::Mutex::new(None),
            db: Mutex::new(db),
            stt_key_validated: tokio::sync::Mutex::new(false),
            recorder,
            dictation: Mutex::new(DictationPhase::Idle),
            dictation_stop: Mutex::new(None),
            dictation_generation: std::sync::atomic::AtomicU64::new(0),
            log_guard: Mutex::new(None),
        }
    }

    /// Park the tracing-appender flush guard for the process lifetime (D-L4).
    /// Called once from the setup hook after the subscriber is installed.
    pub fn set_log_guard(&self, guard: Option<tracing_appender::non_blocking::WorkerGuard>) {
        if let Ok(mut slot) = self.log_guard.lock() {
            *slot = guard;
        }
    }
}
