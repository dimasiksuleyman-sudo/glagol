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

use std::sync::Arc;

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
}

impl AppState {
    /// Construct fresh state with no auth configured.
    pub fn new(http_client: reqwest::Client) -> Self {
        Self {
            http_client,
            salute_auth: tokio::sync::Mutex::new(None),
        }
    }
}
