//! Error types for the SaluteSpeech client.
//!
//! All SaluteSpeech operations return `SaluteResult<T>`, which is
//! `Result<T, SaluteError>`. Errors are classified by category so that
//! callers (e.g. Tauri commands) can produce user-friendly messages
//! and decide retry strategy.

use thiserror::Error;

/// All errors that can occur in the SaluteSpeech client.
#[derive(Error, Debug)]
pub enum SaluteError {
    /// Underlying network failure (DNS, connection refused, TLS handshake, etc.).
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Authentication failed (invalid Authorization Key, revoked credentials).
    /// Maps to HTTP 401 on OAuth endpoint.
    #[error("authentication failed: {message}")]
    Auth { message: String },

    /// API returned an unexpected status code with response body.
    /// Includes the X-Request-ID if present, for Sberbank support tickets.
    #[error("API returned status {status}: {body}")]
    Api { status: u16, body: String },

    /// SaluteSpeech rate limit hit (HTTP 429).
    /// `retry_after_secs` is parsed from the `Retry-After` response header
    /// if present, otherwise defaults to an exponential backoff value.
    #[error("rate limited (429), retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    /// OAuth access token has expired or been revoked.
    /// Callers should call `refresh_token()` and retry the operation once.
    #[error("token expired or invalid")]
    TokenExpired,

    /// Failed to load or parse the embedded НУЦ Минцифры root certificate.
    /// This is a build-time configuration error and should never happen
    /// in a properly built release binary.
    #[error("certificate error: {0}")]
    Certificate(String),

    /// API returned a response we could not parse (malformed JSON,
    /// unexpected schema, missing fields).
    #[error("invalid response: {0}")]
    InvalidResponse(String),

    /// Internal logic error (programmer mistake, unexpected state).
    /// Should be rare; if seen by a user, it's a bug.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Convenience type alias used throughout the SaluteSpeech client.
pub type SaluteResult<T> = Result<T, SaluteError>;
