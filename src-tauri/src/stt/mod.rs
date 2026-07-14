//! Speech-to-text (STT) client for the Dictation feature (Sprint 6).
//!
//! This module is the **provider seam** the rest of the dictation stack is
//! built on. PR1 ships the invisible backend: an OpenAI-compatible HTTP
//! client ([`openai_compat::OpenAiCompatStt`]), the error taxonomy
//! ([`SttError`]), a WAV wrapper ([`wav`]), and the input validators
//! ([`validation`]). Later PRs add the microphone recorder (PR2), the global
//! hotkey + overlay (PR3), and text insertion (PR4) on top of this seam
//! without touching the client.
//!
//! # Why a trait
//!
//! The transcription pipeline (future PRs) is generic over `P: SttProvider`
//! rather than `dyn SttProvider` — native `async fn` in traits (stable since
//! Rust 1.75; our toolchain is 1.94) are **not** dyn-compatible, but we do
//! not need dynamic dispatch. Generics keep the seam zero-cost and let tests
//! substitute a `FakeStt` for the real HTTP client. The embedded
//! `whisper.cpp` backend is a future, additive second arm of [`SttBackend`].
//!
//! # Security
//!
//! Every STT request is issued from Rust only (never the webview) against a
//! user-configured, OpenAI-compatible endpoint. External hosts are
//! https-only; `http://` is accepted for `localhost`/`127.0.0.1`/`::1`
//! exclusively so a locally-hosted whisper server works while an API key
//! (Bearer) never travels in cleartext to a remote host. See
//! [`validation::validate_base_url`].

pub mod openai_compat;
pub mod validation;
pub mod wav;

use std::time::Duration;

use thiserror::Error;

/// A successful transcription result.
///
/// Deliberately minimal for PR1 — the provider contract returns plain text.
/// Word-level timestamps / confidence live behind a richer response format we
/// do not request yet.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Transcript {
    /// The recognised text, exactly as the provider returned it (trimming is
    /// the caller's concern — the pipeline in PR2+ owns post-processing).
    pub text: String,
}

/// Errors that can occur talking to an OpenAI-compatible STT endpoint.
///
/// Classified by category so the command layer can translate each to a
/// concrete, actionable Russian sentence (see
/// `commands::dictation::stt_error_to_user_facing_ru`). The internal
/// `Display` forms stay English so tests can assert on them; the user never
/// sees these strings verbatim.
#[derive(Error, Debug)]
pub enum SttError {
    /// Transport-level failure (DNS, connection refused, TLS, timeout, or a
    /// misconfigured proxy). Wraps the underlying [`reqwest::Error`].
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// HTTP 401 — the API key is missing, wrong, or revoked.
    #[error("authentication failed (401)")]
    Auth,

    /// HTTP 402 — the provider reports the account is out of balance/quota.
    #[error("payment required (402): check your balance with the provider")]
    Balance,

    /// HTTP 429 — rate limited. Carries the parsed `Retry-After` delay when
    /// the provider sent one.
    #[error("rate limited (429)")]
    RateLimited(Option<Duration>),

    /// Any other non-success status (413, 4xx, 5xx). Carries a
    /// human-readable description including the status and a snippet of the
    /// response body.
    #[error("API error: {0}")]
    Api(String),

    /// The request succeeded at the HTTP layer but the body could not be
    /// parsed into the expected `{"text": "..."}` shape.
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}

/// The contract every STT backend implements.
///
/// Native `async fn` in traits — no `async-trait` crate. The returned futures
/// are `Send` for the concrete `reqwest`-backed implementation, which is all
/// the (generic, non-dyn) pipeline needs.
#[allow(async_fn_in_trait)]
pub trait SttProvider {
    /// Transcribe a single WAV clip. `lang` is `Some("ru")`/`Some("en")` to
    /// pin the recognition language, or `None` for auto-detect (the provider
    /// receives no `language` field in that case).
    async fn transcribe(
        &self,
        wav_bytes: Vec<u8>,
        lang: Option<&str>,
    ) -> Result<Transcript, SttError>;

    /// List the models the endpoint exposes. Used as a cheap "is the key +
    /// endpoint + proxy alive" probe — the returned list is secondary to the
    /// fact that the call succeeded.
    async fn list_models(&self) -> Result<Vec<String>, SttError>;
}

/// The set of concrete STT backends.
///
/// PR1 has exactly one arm; the embedded `whisper.cpp` backend will be a
/// second, additive variant post-MVP. Kept as an enum (rather than collapsing
/// to the single struct) so that growth is a purely additive change and the
/// dispatch site is a single, greppable `match`.
pub enum SttBackend {
    /// Any OpenAI-compatible HTTP endpoint (AITunnel, Groq, a local
    /// whisper server, …).
    OpenAiCompat(openai_compat::OpenAiCompatStt),
}

impl SttProvider for SttBackend {
    async fn transcribe(
        &self,
        wav_bytes: Vec<u8>,
        lang: Option<&str>,
    ) -> Result<Transcript, SttError> {
        match self {
            SttBackend::OpenAiCompat(inner) => inner.transcribe(wav_bytes, lang).await,
        }
    }

    async fn list_models(&self) -> Result<Vec<String>, SttError> {
        match self {
            SttBackend::OpenAiCompat(inner) => inner.list_models().await,
        }
    }
}
