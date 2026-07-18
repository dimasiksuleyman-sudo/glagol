//! OpenAI-compatible STT client.
//!
//! Mirrors the shape of `salute::synthesize::SynthesisClient`: a cheap,
//! cloneable struct wrapping a shared [`reqwest::Client`], with a
//! `with_base_url` constructor for `mockito` tests. It speaks the stable
//! `/audio/transcriptions` + `/models` contract shared by OVH, Lemonfox,
//! Groq, AITunnel and localhost whisper servers (Vox-Box / Speaches).

use std::time::Duration;

use reqwest::{header::HeaderMap, multipart, Client, StatusCode};
use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::stt::{SttError, SttProvider, Transcript};

/// Request timeout for a single transcription. A dictated phrase uploads in
/// one batch; 1–3 s latency is normal, but a slow network or a proxy hop can
/// spike, so we leave generous headroom. Not user-configurable.
const TRANSCRIBE_TIMEOUT: Duration = Duration::from_secs(90);

/// Request timeout for the `GET /models` liveness probe. Much shorter than
/// [`TRANSCRIBE_TIMEOUT`] — there is no upload, so a slow answer means a dead
/// endpoint or proxy, and the Settings «Проверить» button must fail fast
/// rather than hang. The reqwest client built for STT has no default timeout,
/// so this per-request cap is what bounds the probe.
const LIST_MODELS_TIMEOUT: Duration = Duration::from_secs(20);

/// How many bytes of an error response body to keep in an [`SttError::Api`]
/// message — enough to be diagnostic without dumping a whole HTML error page.
const ERROR_BODY_SNIPPET_LEN: usize = 500;

/// An OpenAI-compatible speech-to-text client.
///
/// `base_url` is stored already normalised (no trailing slash); endpoints are
/// appended as `{base_url}/audio/transcriptions` and `{base_url}/models`.
/// `api_key` is optional so a keyless localhost server works.
#[derive(Debug, Clone)]
pub struct OpenAiCompatStt {
    client: Client,
    base_url: String,
    model: String,
    api_key: Option<String>,
    /// Optional Whisper `prompt` biasing hint — a short vocabulary of proper
    /// nouns the model otherwise mis-hears (kickoff D8). `None` by default;
    /// dictation sets it via [`OpenAiCompatStt::with_prompt`]. The Settings
    /// key-check probe leaves it `None` (silence needs no vocabulary).
    prompt: Option<String>,
}

impl OpenAiCompatStt {
    /// Build a client from an explicit [`reqwest::Client`] (production passes
    /// one configured with the desired proxy / timeout).
    ///
    /// `base_url` is normalised by stripping a single trailing `/` so the
    /// preset `https://api.aitunnel.ru/v1` and a hand-typed
    /// `https://api.aitunnel.ru/v1/` resolve identically.
    pub fn new(
        client: Client,
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: Option<String>,
    ) -> Self {
        Self {
            client,
            base_url: normalize_base_url(&base_url.into()),
            model: model.into(),
            api_key,
            prompt: None,
        }
    }

    /// Attach a Whisper `prompt` biasing hint (kickoff D8). A trimmed-empty hint
    /// clears it back to `None` so an empty setting never sends a blank field.
    /// Builder-style so [`OpenAiCompatStt::new`]'s four-argument shape is
    /// unchanged for the Settings key-check path, which needs no vocabulary.
    pub fn with_prompt(mut self, prompt: Option<String>) -> Self {
        self.prompt = prompt.filter(|p| !p.trim().is_empty());
        self
    }

    /// Test-only constructor: a default `reqwest::Client`, a placeholder model
    /// and a placeholder key, targeting `base_url` (a `mockito` server URL).
    /// Production code uses [`OpenAiCompatStt::new`] with a real client.
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::new(
            Client::new(),
            base_url,
            "whisper-1",
            Some("test-key".to_string()),
        )
    }

    fn transcribe_url(&self) -> String {
        format!("{}/audio/transcriptions", self.base_url)
    }

    fn models_url(&self) -> String {
        format!("{}/models", self.base_url)
    }

    /// Attach the `Authorization: Bearer …` header when a key is configured.
    fn with_auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) => req.header("Authorization", format!("Bearer {key}")),
            None => req,
        }
    }

    /// `POST {base_url}/audio/transcriptions` — multipart upload of one WAV
    /// clip. See [`SttProvider::transcribe`] for the `lang` semantics.
    pub async fn transcribe(
        &self,
        wav_bytes: Vec<u8>,
        lang: Option<&str>,
    ) -> Result<Transcript, SttError> {
        let byte_len = wav_bytes.len();

        // The filename is NOT optional: some providers key format detection on
        // the `.wav` extension of the `file` part.
        let file_part = multipart::Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| {
                SttError::InvalidResponse(format!("failed to build multipart part: {e}"))
            })?;

        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("response_format", "json");

        // `None` (auto) → omit the language field entirely.
        if let Some(lang) = lang {
            form = form.text("language", lang.to_string());
        }

        // Whisper `prompt` biasing hint (D8) — only sent when configured.
        if let Some(prompt) = &self.prompt {
            form = form.text("prompt", prompt.clone());
        }

        debug!(
            bytes = byte_len,
            model = %self.model,
            lang = ?lang,
            "stt transcribe request"
        );

        let request = self
            .with_auth(
                self.client
                    .post(self.transcribe_url())
                    .timeout(TRANSCRIBE_TIMEOUT),
            )
            .multipart(form);

        let response = request.send().await.map_err(|e| {
            warn!(error = %e, "stt transcribe network error");
            SttError::from(e)
        })?;

        let status = response.status();
        if status.is_success() {
            let body = response.text().await.map_err(SttError::from)?;
            let transcript = parse_transcript(&body)?;
            info!(
                chars = transcript.text.chars().count(),
                "stt transcribe success"
            );
            return Ok(transcript);
        }

        // Parse Retry-After (borrowing headers) before consuming the body.
        let retry_after = response_retry_after(response.headers());
        let body = response.text().await.unwrap_or_default();
        Err(map_error_status(status, &retry_after, body))
    }

    /// `GET {base_url}/models` — a cheap liveness probe for the key, endpoint
    /// and proxy. Returns the model ids on success; an unfamiliar-but-200 body
    /// still counts as success (empty list) because the auth check passed.
    pub async fn list_models(&self) -> Result<Vec<String>, SttError> {
        debug!(url = %self.models_url(), "stt list_models request");

        let response = self
            .with_auth(
                self.client
                    .get(self.models_url())
                    .timeout(LIST_MODELS_TIMEOUT),
            )
            .send()
            .await
            .map_err(|e| {
                warn!(error = %e, "stt list_models network error");
                SttError::from(e)
            })?;

        let status = response.status();
        if !status.is_success() {
            let retry_after = response_retry_after(response.headers());
            let body = response.text().await.unwrap_or_default();
            return Err(map_error_status(status, &retry_after, body));
        }

        let body = response.text().await.map_err(SttError::from)?;
        Ok(parse_model_ids(&body))
    }
}

impl SttProvider for OpenAiCompatStt {
    async fn transcribe(
        &self,
        wav_bytes: Vec<u8>,
        lang: Option<&str>,
    ) -> Result<Transcript, SttError> {
        // Explicit path so the inherent method is called (no recursion).
        OpenAiCompatStt::transcribe(self, wav_bytes, lang).await
    }

    async fn list_models(&self) -> Result<Vec<String>, SttError> {
        OpenAiCompatStt::list_models(self).await
    }
}

/// Strip a single trailing `/` so `{base}/audio/transcriptions` never doubles
/// the separator. Presets already carry the `/v1` suffix; we do not add one.
fn normalize_base_url(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_string()
}

/// Parse the `{"text": "..."}` success body into a [`Transcript`].
fn parse_transcript(body: &str) -> Result<Transcript, SttError> {
    #[derive(Deserialize)]
    struct Resp {
        text: String,
    }

    let resp: Resp = serde_json::from_str(body).map_err(|e| {
        SttError::InvalidResponse(format!("expected {{\"text\": ...}}, got parse error: {e}"))
    })?;
    Ok(Transcript { text: resp.text })
}

/// Best-effort extraction of model ids from a `GET /models` body. Returns an
/// empty vec (not an error) for an unfamiliar shape: a 200 already proved the
/// key/endpoint work, which is all the probe cares about.
fn parse_model_ids(body: &str) -> Vec<String> {
    #[derive(Deserialize)]
    struct ModelsResp {
        data: Vec<Model>,
    }
    #[derive(Deserialize)]
    struct Model {
        id: String,
    }

    match serde_json::from_str::<ModelsResp>(body) {
        Ok(resp) => resp.data.into_iter().map(|m| m.id).collect(),
        Err(_) => Vec::new(),
    }
}

/// Read and parse the `Retry-After` header (integer seconds form) into a
/// [`Duration`]. HTTP-date form is not parsed (providers use seconds here).
fn response_retry_after(headers: &HeaderMap) -> Option<Duration> {
    headers
        .get("Retry-After")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

/// Map a non-success status + body to the appropriate [`SttError`].
fn map_error_status(status: StatusCode, retry_after: &Option<Duration>, body: String) -> SttError {
    match status {
        StatusCode::UNAUTHORIZED => SttError::Auth,
        StatusCode::PAYMENT_REQUIRED => SttError::Balance,
        StatusCode::TOO_MANY_REQUESTS => SttError::RateLimited(*retry_after),
        StatusCode::PAYLOAD_TOO_LARGE => SttError::TooLarge,
        other => {
            let mut snippet = body;
            if snippet.chars().count() > ERROR_BODY_SNIPPET_LEN {
                snippet = snippet.chars().take(ERROR_BODY_SNIPPET_LEN).collect();
            }
            SttError::Api(format!("HTTP {}: {}", other.as_u16(), snippet))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client_for(server: &mockito::Server) -> OpenAiCompatStt {
        OpenAiCompatStt::with_base_url(server.url())
    }

    #[test]
    fn normalize_base_url_strips_trailing_slash() {
        assert_eq!(normalize_base_url("https://x/v1/"), "https://x/v1");
        assert_eq!(normalize_base_url("https://x/v1"), "https://x/v1");
        assert_eq!(normalize_base_url("  https://x/v1  "), "https://x/v1");
    }

    #[test]
    fn new_normalizes_base_url_for_endpoints() {
        let c = OpenAiCompatStt::new(Client::new(), "https://x/v1/", "m", None);
        assert_eq!(c.transcribe_url(), "https://x/v1/audio/transcriptions");
        assert_eq!(c.models_url(), "https://x/v1/models");
    }

    #[tokio::test]
    async fn transcribe_success_returns_text() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/audio/transcriptions")
            .match_header("authorization", "Bearer test-key")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"text":"привет мир"}"#)
            .create_async()
            .await;

        let client = client_for(&server);
        let out = client
            .transcribe(vec![1, 2, 3], Some("ru"))
            .await
            .expect("success");
        assert_eq!(out.text, "привет мир");
    }

    #[tokio::test]
    async fn transcribe_sends_expected_multipart_fields() {
        let mut server = mockito::Server::new_async().await;
        // Assert the multipart body carries file(+filename), model,
        // response_format and language parts.
        let _m = server
            .mock("POST", "/audio/transcriptions")
            .match_body(mockito::Matcher::AllOf(vec![
                mockito::Matcher::Regex("name=\"file\"".to_string()),
                mockito::Matcher::Regex("filename=\"audio.wav\"".to_string()),
                mockito::Matcher::Regex("name=\"model\"".to_string()),
                mockito::Matcher::Regex("whisper-1".to_string()),
                mockito::Matcher::Regex("name=\"response_format\"".to_string()),
                mockito::Matcher::Regex("name=\"language\"".to_string()),
            ]))
            .with_status(200)
            .with_body(r#"{"text":"ok"}"#)
            .create_async()
            .await;

        let client = client_for(&server);
        client
            .transcribe(vec![0u8; 8], Some("ru"))
            .await
            .expect("multipart request accepted");
    }

    #[tokio::test]
    async fn transcribe_sends_prompt_field_when_configured() {
        // D8: a configured Whisper prompt must ride the multipart body so the
        // provider biases toward the app's proper-noun vocabulary.
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/audio/transcriptions")
            .match_body(mockito::Matcher::AllOf(vec![
                mockito::Matcher::Regex("name=\"prompt\"".to_string()),
                mockito::Matcher::Regex("Глагол".to_string()),
            ]))
            .with_status(200)
            .with_body(r#"{"text":"ok"}"#)
            .create_async()
            .await;

        let client = client_for(&server).with_prompt(Some("Глагол, Привезём.".to_string()));
        client
            .transcribe(vec![0u8; 4], Some("ru"))
            .await
            .expect("prompt-carrying request accepted");
    }

    #[test]
    fn with_prompt_ignores_blank_hint() {
        // A whitespace-only prompt must clear to None so we never send an empty
        // `prompt` field (which some providers reject).
        let c = OpenAiCompatStt::with_base_url("https://x/v1").with_prompt(Some("   ".to_string()));
        assert!(c.prompt.is_none());
        let c = c.with_prompt(Some("Глагол".to_string()));
        assert_eq!(c.prompt.as_deref(), Some("Глагол"));
    }

    #[tokio::test]
    async fn transcribe_auto_language_omits_language_field() {
        let mut server = mockito::Server::new_async().await;
        // With lang=None the body must NOT contain a language part.
        let _m = server
            .mock("POST", "/audio/transcriptions")
            .match_body(mockito::Matcher::AllOf(vec![mockito::Matcher::Regex(
                "name=\"model\"".to_string(),
            )]))
            .with_status(200)
            .with_body(r#"{"text":"ok"}"#)
            .create_async()
            .await;

        let client = client_for(&server);
        let out = client.transcribe(vec![0u8; 4], None).await.expect("ok");
        assert_eq!(out.text, "ok");
        // Note: mockito cannot easily assert *absence*; the omission logic is
        // covered structurally by the `if let Some(lang)` guard and the
        // presence-test above.
    }

    #[tokio::test]
    async fn transcribe_401_maps_to_auth() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/audio/transcriptions")
            .with_status(401)
            .with_body("unauthorized")
            .create_async()
            .await;

        let client = client_for(&server);
        match client.transcribe(vec![1], Some("ru")).await.unwrap_err() {
            SttError::Auth => {}
            other => panic!("expected Auth, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn transcribe_402_maps_to_balance() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/audio/transcriptions")
            .with_status(402)
            .with_body("payment required")
            .create_async()
            .await;

        let client = client_for(&server);
        match client.transcribe(vec![1], Some("ru")).await.unwrap_err() {
            SttError::Balance => {}
            other => panic!("expected Balance, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn transcribe_429_parses_retry_after() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/audio/transcriptions")
            .with_status(429)
            .with_header("Retry-After", "17")
            .with_body("slow down")
            .create_async()
            .await;

        let client = client_for(&server);
        match client.transcribe(vec![1], Some("ru")).await.unwrap_err() {
            SttError::RateLimited(Some(d)) => assert_eq!(d, Duration::from_secs(17)),
            other => panic!("expected RateLimited(Some(17s)), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn transcribe_429_without_header_has_no_duration() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/audio/transcriptions")
            .with_status(429)
            .with_body("slow down")
            .create_async()
            .await;

        let client = client_for(&server);
        match client.transcribe(vec![1], Some("ru")).await.unwrap_err() {
            SttError::RateLimited(None) => {}
            other => panic!("expected RateLimited(None), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn transcribe_413_maps_to_too_large() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/audio/transcriptions")
            .with_status(413)
            .with_body("too big")
            .create_async()
            .await;

        let client = client_for(&server);
        match client.transcribe(vec![1], Some("ru")).await.unwrap_err() {
            SttError::TooLarge => {}
            other => panic!("expected TooLarge, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn transcribe_500_maps_to_api_with_body() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/audio/transcriptions")
            .with_status(500)
            .with_body("boom internal")
            .create_async()
            .await;

        let client = client_for(&server);
        match client.transcribe(vec![1], Some("ru")).await.unwrap_err() {
            SttError::Api(msg) => {
                assert!(msg.contains("500"), "status in message: {msg}");
                assert!(msg.contains("boom internal"), "body in message: {msg}");
            }
            other => panic!("expected Api, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn transcribe_malformed_json_maps_to_invalid_response() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/audio/transcriptions")
            .with_status(200)
            .with_body("not json at all")
            .create_async()
            .await;

        let client = client_for(&server);
        match client.transcribe(vec![1], Some("ru")).await.unwrap_err() {
            SttError::InvalidResponse(_) => {}
            other => panic!("expected InvalidResponse, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_models_success_returns_ids() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/models")
            .match_header("authorization", "Bearer test-key")
            .with_status(200)
            .with_body(r#"{"object":"list","data":[{"id":"whisper-1"},{"id":"whisper-large-v3"}]}"#)
            .create_async()
            .await;

        let client = client_for(&server);
        let ids = client.list_models().await.expect("ok");
        assert_eq!(ids, vec!["whisper-1", "whisper-large-v3"]);
    }

    #[tokio::test]
    async fn list_models_unfamiliar_200_body_is_empty_not_error() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/models")
            .with_status(200)
            .with_body(r#"{"weird":"shape"}"#)
            .create_async()
            .await;

        let client = client_for(&server);
        let ids = client.list_models().await.expect("200 counts as success");
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn list_models_401_maps_to_auth() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/models")
            .with_status(401)
            .with_body("nope")
            .create_async()
            .await;

        let client = client_for(&server);
        match client.list_models().await.unwrap_err() {
            SttError::Auth => {}
            other => panic!("expected Auth, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn no_api_key_omits_authorization_header() {
        let mut server = mockito::Server::new_async().await;
        // Mock only matches when NO authorization header is present.
        let _m = server
            .mock("GET", "/models")
            .match_header("authorization", mockito::Matcher::Missing)
            .with_status(200)
            .with_body(r#"{"data":[]}"#)
            .create_async()
            .await;

        let client = OpenAiCompatStt::new(Client::new(), server.url(), "whisper-1", None);
        client
            .list_models()
            .await
            .expect("keyless request should be accepted (no auth header sent)");
    }
}
