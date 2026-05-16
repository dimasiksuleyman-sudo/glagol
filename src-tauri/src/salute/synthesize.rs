//! Speech synthesis via SaluteSpeech sync REST API.
//!
//! This module wraps the `POST /rest/v1/text:synthesize` endpoint, which
//! converts UTF-8 text (up to 4000 characters) into binary audio (WAV/OPUS/PCM).
//!
//! # Quick example (planned, full implementation in next step)
//!
//! ```ignore
//! use glagol_lib::salute::{auth::SaluteAuth, http, synthesize::{SynthesisClient, VoiceId}};
//!
//! let client = http::build_client()?;
//! let auth = SaluteAuth::new(client.clone(), auth_key);
//! let synth = SynthesisClient::new(client);
//!
//! let token = auth.get_token().await?;
//! let wav_bytes = synth.synthesize(&token, "Привет, мир!", VoiceId::Natalia).await?;
//! std::fs::write("hello.wav", wav_bytes)?;
//! ```
//!
//! # Error mapping
//!
//! - 200 OK → `Ok(wav_bytes)`
//! - 400 → [`SaluteError::Api`] (usually text too long or malformed SSML)
//! - 401 → [`SaluteError::TokenExpired`] — see note in [`SynthesisClient::synthesize`]
//! - 429 → [`SaluteError::RateLimited`] with `retry_after_secs` from header
//! - 4xx/5xx → [`SaluteError::Api`]

use crate::salute::errors::{SaluteError, SaluteResult};
use crate::salute::http;
use reqwest::{Client, StatusCode};
use tracing::{debug, info, warn};

/// Default synthesis endpoint (production Sberbank).
const DEFAULT_SYNTHESIS_URL: &str = "https://smartspeech.sber.ru/rest/v1/text:synthesize";

/// Default fallback if `Retry-After` header is missing or unparseable.
/// Sberbank's rate-limit window for the PERS tier is typically ~60 seconds.
const DEFAULT_RETRY_AFTER_SECS: u64 = 60;

/// Available SaluteSpeech voices for Russian synthesis.
///
/// We expose a typed enum (rather than free-form strings) so that:
/// - Callers cannot pass invalid voice IDs.
/// - Tauri commands return a closed set serializable to the frontend.
/// - Future addition/removal of voices is a breaking change visible in code review.
///
/// `Kira` is intentionally excluded — it is English-only (en-US) and Glagol
/// is focused on Russian. Add it later if EN support becomes a feature.
///
/// All voices use the `_24000` (24 kHz) sample rate variant for quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VoiceId {
    /// Наталья — female, default voice. Supports stress marks (`+` before vowel).
    Natalia,
    /// Борис — male.
    Boris,
    /// Марфа — female.
    Marfa,
    /// Тарас — male.
    Taras,
    /// Александра — female.
    Alexandra,
    /// Сергей — male. Supports stress marks (`+` before vowel).
    Sergey,
}

impl VoiceId {
    /// Convert to the API identifier string expected by SaluteSpeech
    /// `voice` query parameter.
    pub fn as_api_id(&self) -> &'static str {
        match self {
            VoiceId::Natalia => "Nec_24000",
            VoiceId::Boris => "Bys_24000",
            VoiceId::Marfa => "May_24000",
            VoiceId::Taras => "Tur_24000",
            VoiceId::Alexandra => "Ost_24000",
            VoiceId::Sergey => "Pon_24000",
        }
    }
}

/// Synthesis client for SaluteSpeech.
///
/// Construct once per app session (or even at app startup) and reuse;
/// the underlying `reqwest::Client` is cheap to clone and connection-pools internally.
///
/// This client is **stateless** — it does not cache anything. Token caching
/// is the responsibility of [`crate::salute::auth::SaluteAuth`].
#[derive(Debug, Clone)]
pub struct SynthesisClient {
    /// Shared HTTP client (already configured with TLS pinning).
    client: Client,

    /// Synthesis endpoint URL. Defaults to Sberbank production; can be
    /// overridden via [`SynthesisClient::with_base_url`] for tests.
    synthesis_url: String,
}

impl SynthesisClient {
    /// Create a synthesis client targeting the Sberbank production endpoint.
    ///
    /// # Arguments
    ///
    /// - `client` — pre-built [`reqwest::Client`] with TLS pinning (use
    ///   [`crate::salute::http::build_client`]).
    pub fn new(client: Client) -> Self {
        Self::with_base_url(client, DEFAULT_SYNTHESIS_URL.to_string())
    }

    /// Create a synthesis client with a custom endpoint URL.
    ///
    /// Used in unit tests with `mockito`, which serves HTTP (not HTTPS)
    /// on `127.0.0.1`. Production code should use [`SynthesisClient::new`].
    pub fn with_base_url(client: Client, synthesis_url: String) -> Self {
        Self {
            client,
            synthesis_url,
        }
    }

    /// Synthesize the given text into audio bytes (WAV16 format).
    ///
    /// **Not yet implemented** — will be filled in Step 4.
    ///
    /// # Note on 401 handling
    ///
    /// On 401 here (after auth.rs already produced a token), we map to
    /// [`SaluteError::TokenExpired`] (not [`SaluteError::Auth`]). Rationale:
    /// at this point the `auth_key` already succeeded in producing a token,
    /// so 401 here means the *access_token* is bad/expired. Callers can
    /// react by calling [`crate::salute::auth::SaluteAuth::refresh_token`]
    /// and retrying.
    ///
    /// Compare: 401 in `auth.rs` means the `auth_key` itself is invalid → `Auth`.
    pub async fn synthesize(
        &self,
        access_token: &str,
        text: &str,
        voice: VoiceId,
    ) -> SaluteResult<Vec<u8>> {
        let rquid = http::new_rquid();
        debug!(
            rquid = %rquid,
            voice = ?voice,
            text_len = text.len(),
            "synthesize request"
        );

        let response = self
            .client
            .post(&self.synthesis_url)
            .query(&[("format", "wav16"), ("voice", voice.as_api_id())])
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/text")
            .header("RqUID", &rquid)
            .body(text.to_string())
            .send()
            .await
            .map_err(|e| {
                warn!(rquid = %rquid, error = %e, "synthesize network error");
                SaluteError::from(e)
            })?;

        let status = response.status();

        // Success path: read binary body and return.
        if status.is_success() {
            let bytes = response.bytes().await.map_err(|e| {
                SaluteError::InvalidResponse(format!("failed to read response body: {}", e))
            })?;
            info!(
                rquid = %rquid,
                bytes_len = bytes.len(),
                "synthesize success"
            );
            return Ok(bytes.to_vec());
        }

        // Error path: parse Retry-After header (if 429) BEFORE consuming body.
        let retry_after_secs = if status == StatusCode::TOO_MANY_REQUESTS {
            response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(DEFAULT_RETRY_AFTER_SECS)
        } else {
            DEFAULT_RETRY_AFTER_SECS // unused for non-429 statuses
        };

        // Consume body as text (might be JSON, plain, or HTML — we don't care).
        let body = response.text().await.unwrap_or_default();

        warn!(
            rquid = %rquid,
            status = %status,
            body_len = body.len(),
            "synthesize non-success response"
        );

        // Map status to SaluteError variant.
        // Note: 401 here maps to TokenExpired (not Auth) — see doc comment above.
        Err(match status {
            StatusCode::UNAUTHORIZED => SaluteError::TokenExpired,
            StatusCode::TOO_MANY_REQUESTS => SaluteError::RateLimited { retry_after_secs },
            _ => SaluteError::Api {
                status: status.as_u16(),
                body,
            },
        })
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_id_api_mapping() {
        // Each VoiceId must map to its exact Sberbank API identifier.
        // If Sberbank renames a voice, this test will fail and force a code review.
        assert_eq!(VoiceId::Natalia.as_api_id(), "Nec_24000");
        assert_eq!(VoiceId::Boris.as_api_id(), "Bys_24000");
        assert_eq!(VoiceId::Marfa.as_api_id(), "May_24000");
        assert_eq!(VoiceId::Taras.as_api_id(), "Tur_24000");
        assert_eq!(VoiceId::Alexandra.as_api_id(), "Ost_24000");
        assert_eq!(VoiceId::Sergey.as_api_id(), "Pon_24000");
    }

    #[test]
    fn test_new_uses_default_url() {
        let client = crate::salute::http::build_client().expect("client builds");
        let synth = SynthesisClient::new(client);
        assert_eq!(synth.synthesis_url, DEFAULT_SYNTHESIS_URL);
    }

    #[test]
    fn test_with_base_url_overrides_endpoint() {
        let client = crate::salute::http::build_client().expect("client builds");
        let custom_url = "http://localhost:1234/mock/synthesize".to_string();
        let synth = SynthesisClient::with_base_url(client, custom_url.clone());
        assert_eq!(synth.synthesis_url, custom_url);
    }
    // ========================================================================
    // Synthesis flow tests with mockito (no real network calls to Sberbank)
    // ========================================================================

    /// Minimal valid WAV header (44 bytes) for binary round-trip tests.
    /// We do not need real audio — just bytes that prove the response body
    /// passes through unmodified.
    const MINIMAL_WAV_HEADER: &[u8] = &[
        0x52, 0x49, 0x46, 0x46, // "RIFF"
        0x24, 0x00, 0x00, 0x00, // file size minus 8
        0x57, 0x41, 0x56, 0x45, // "WAVE"
        0x66, 0x6d, 0x74, 0x20, // "fmt "
        0x10, 0x00, 0x00, 0x00, // fmt chunk size (16)
        0x01, 0x00, // PCM format
        0x01, 0x00, // mono
        0x80, 0x3e, 0x00, 0x00, // 16000 Hz
        0x00, 0x7d, 0x00, 0x00, // byte rate
        0x02, 0x00, // block align
        0x10, 0x00, // 16 bits per sample
        0x64, 0x61, 0x74, 0x61, // "data"
        0x00, 0x00, 0x00, 0x00, // data size (empty)
    ];

    /// Helper: build a SynthesisClient wired to a mockito server.
    fn make_test_client(server: &mockito::Server) -> SynthesisClient {
        let client = http::build_client().expect("client builds");
        let synthesis_url = format!("{}/rest/v1/text:synthesize", server.url());
        SynthesisClient::with_base_url(client, synthesis_url)
    }

    #[tokio::test]
    async fn test_synthesize_success_returns_wav_bytes() {
        let mut server = mockito::Server::new_async().await;

        // Expect exactly the headers/query our code sends.
        let _mock = server
            .mock("POST", "/rest/v1/text:synthesize")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("format".into(), "wav16".into()),
                mockito::Matcher::UrlEncoded("voice".into(), "Nec_24000".into()),
            ]))
            .match_header("authorization", "Bearer fake_token_123")
            .match_header("content-type", "application/text")
            .with_status(200)
            .with_header("content-type", "audio/wav")
            .with_body(MINIMAL_WAV_HEADER)
            .create_async()
            .await;

        let synth = make_test_client(&server);
        let result = synth
            .synthesize("fake_token_123", "Привет, мир!", VoiceId::Natalia)
            .await;

        assert!(result.is_ok(), "expected success, got: {:?}", result);
        let bytes = result.unwrap();
        assert_eq!(
            bytes.as_slice(),
            MINIMAL_WAV_HEADER,
            "WAV bytes should round-trip through the client unchanged"
        );
    }

    #[tokio::test]
    async fn test_synthesize_401_returns_token_expired() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", "/rest/v1/text:synthesize")
            .match_query(mockito::Matcher::Any)
            .with_status(401)
            .with_body(r#"{"error":"token_expired"}"#)
            .create_async()
            .await;

        let synth = make_test_client(&server);
        let result = synth
            .synthesize("expired_token", "test", VoiceId::Boris)
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            SaluteError::TokenExpired => { /* expected */ }
            other => panic!("expected TokenExpired, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_synthesize_400_returns_api_error_with_body() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", "/rest/v1/text:synthesize")
            .match_query(mockito::Matcher::Any)
            .with_status(400)
            .with_body(r#"{"error":"text_too_long","max_chars":4000}"#)
            .create_async()
            .await;

        let synth = make_test_client(&server);
        let result = synth
            .synthesize("token", "very long text", VoiceId::Marfa)
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            SaluteError::Api { status, body } => {
                assert_eq!(status, 400);
                assert!(
                    body.contains("text_too_long"),
                    "API error body should contain server response, got: {}",
                    body
                );
            }
            other => panic!("expected Api error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_synthesize_429_uses_retry_after_header() {
        let mut server = mockito::Server::new_async().await;

        // Server returns 429 with explicit Retry-After: 42 seconds.
        let _mock = server
            .mock("POST", "/rest/v1/text:synthesize")
            .match_query(mockito::Matcher::Any)
            .with_status(429)
            .with_header("Retry-After", "42")
            .with_body("rate limited")
            .create_async()
            .await;

        let synth = make_test_client(&server);
        let result = synth.synthesize("token", "text", VoiceId::Taras).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            SaluteError::RateLimited { retry_after_secs } => {
                assert_eq!(
                    retry_after_secs, 42,
                    "should parse Retry-After header value"
                );
            }
            other => panic!("expected RateLimited, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_synthesize_429_without_header_uses_default() {
        let mut server = mockito::Server::new_async().await;

        // Server returns 429 WITHOUT Retry-After header.
        let _mock = server
            .mock("POST", "/rest/v1/text:synthesize")
            .match_query(mockito::Matcher::Any)
            .with_status(429)
            .with_body("rate limited")
            .create_async()
            .await;

        let synth = make_test_client(&server);
        let result = synth.synthesize("token", "text", VoiceId::Alexandra).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            SaluteError::RateLimited { retry_after_secs } => {
                assert_eq!(
                    retry_after_secs, DEFAULT_RETRY_AFTER_SECS,
                    "missing Retry-After should fall back to default"
                );
            }
            other => panic!("expected RateLimited, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_synthesize_500_returns_api_error() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", "/rest/v1/text:synthesize")
            .match_query(mockito::Matcher::Any)
            .with_status(500)
            .with_body("Internal Server Error")
            .create_async()
            .await;

        let synth = make_test_client(&server);
        let result = synth.synthesize("token", "text", VoiceId::Sergey).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            SaluteError::Api { status, body } => {
                assert_eq!(status, 500);
                assert!(
                    body.contains("Internal Server Error"),
                    "500 error should preserve server response"
                );
            }
            other => panic!("expected Api error, got: {:?}", other),
        }
    }
}
