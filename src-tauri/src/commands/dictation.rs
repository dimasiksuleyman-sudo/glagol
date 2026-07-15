//! Tauri commands for the Dictation (STT) feature — Sprint 6 PR1.
//!
//! This is the command surface behind the Settings → «Диктовка (STT)» section:
//!
//! - [`get_stt_settings`] / [`save_stt_settings`] — read/write the non-secret
//!   provider configuration (`base_url`, `model`, `proxy`, `language`) in the
//!   `app_settings` table.
//! - [`set_stt_key`] / [`delete_stt_key`] / [`has_stt_key`] — manage the
//!   provider API key in the OS keyring (a keyless local server is valid).
//! - [`test_stt_key`] — validate the whole chain (key + endpoint + proxy)
//!   against the live provider, cache-first with a `force` bypass mirroring
//!   [`crate::commands::credentials::test_credentials`].
//!
//! Every public command is a thin wrapper over an `*_impl` function taking
//! plain values so the logic is unit-testable without a Tauri runtime.
//!
//! Security (CLAUDE.md invariant #3, relaxed for STT): all provider HTTP is
//! issued here in Rust against a user-configured OpenAI-compatible endpoint;
//! the webview never talks to the provider directly. External endpoints are
//! https-only (see [`crate::stt::validation`]).

use reqwest::{Client, Proxy};
use rusqlite::Connection;
use serde::Serialize;

use crate::db::repository;
use crate::secrets::keyring::{self, KeyringError};
use crate::state::AppState;
use crate::stt::openai_compat::OpenAiCompatStt;
use crate::stt::{validation, wav, SttError};

// ── Settings keys + defaults ───────────────────────────────────────────

const KEY_BASE_URL: &str = "stt_base_url";
const KEY_MODEL: &str = "stt_model";
const KEY_PROXY: &str = "stt_proxy";
const KEY_LANGUAGE: &str = "stt_language";

/// First-run default endpoint: AITunnel always works without a system VPN and
/// costs kopecks per minute (see kickoff D4).
const DEFAULT_BASE_URL: &str = "https://api.aitunnel.ru/v1";
const DEFAULT_MODEL: &str = "whisper-large-v3-turbo";
/// Default recognition language. `ru` (not `auto`) anchors Whisper against
/// language hallucination on the short phrases dictation produces (D11).
const DEFAULT_LANGUAGE: &str = "ru";

/// Sample rate of the silence probe sent when a provider does not expose
/// `/models`. 16 kHz is Whisper's native rate; the content is silence so the
/// exact value only affects the byte count.
const PROBE_SAMPLE_RATE: u32 = 16_000;
const PROBE_DURATION_MS: u32 = 500;

// ── Settings DTO ───────────────────────────────────────────────────────

/// Non-secret STT configuration returned to the Settings UI. Field names are
/// serialized as-is (snake_case), matching the `DocumentRecord` / `UsageInfo`
/// convention on the IPC boundary. The API key is deliberately absent — it
/// lives in the keyring and never crosses IPC on a read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SttSettings {
    pub base_url: String,
    pub model: String,
    /// Empty string when no proxy is configured.
    pub proxy: String,
    /// `"ru"` | `"en"` | `"auto"`.
    pub language: String,
}

// ── Tauri command wrappers ─────────────────────────────────────────────

#[tauri::command]
pub async fn get_stt_settings(state: tauri::State<'_, AppState>) -> Result<SttSettings, String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
    get_stt_settings_impl(&conn)
        .map_err(|e| format!("Не удалось прочитать настройки диктовки: {e}"))
}

#[tauri::command]
pub async fn save_stt_settings(
    state: tauri::State<'_, AppState>,
    base_url: String,
    model: String,
    proxy: String,
    language: String,
) -> Result<(), String> {
    let updated_at = chrono::Utc::now().timestamp_millis();
    save_stt_settings_impl(&state, &base_url, &model, &proxy, &language, updated_at).await
}

#[tauri::command]
pub async fn set_stt_key(state: tauri::State<'_, AppState>, key: String) -> Result<(), String> {
    set_stt_key_impl(&state, &key).await
}

#[tauri::command]
pub async fn delete_stt_key(state: tauri::State<'_, AppState>) -> Result<(), String> {
    delete_stt_key_impl(&state).await
}

#[tauri::command]
pub async fn has_stt_key() -> Result<bool, String> {
    has_stt_key_impl()
}

#[tauri::command]
pub async fn test_stt_key(state: tauri::State<'_, AppState>, force: bool) -> Result<(), String> {
    test_stt_key_impl(&state, force).await
}

// ── Impl functions (unit-testable) ─────────────────────────────────────

/// Read the persisted STT settings, substituting defaults for any unset key.
pub(crate) fn get_stt_settings_impl(conn: &Connection) -> rusqlite::Result<SttSettings> {
    Ok(SttSettings {
        base_url: repository::get_setting(conn, KEY_BASE_URL)?
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
        model: repository::get_setting(conn, KEY_MODEL)?
            .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
        proxy: repository::get_setting(conn, KEY_PROXY)?.unwrap_or_default(),
        language: repository::get_setting(conn, KEY_LANGUAGE)?
            .unwrap_or_else(|| DEFAULT_LANGUAGE.to_string()),
    })
}

/// Validate + persist the STT settings, then invalidate the process-lifetime
/// "key validated" flag — a new endpoint/proxy/model must be re-checked, so a
/// stale `true` would otherwise let the mount-time probe report a config it
/// never actually validated. The persistence itself is transaction-wrapped in
/// [`persist_stt_settings`].
pub(crate) async fn save_stt_settings_impl(
    state: &AppState,
    base_url: &str,
    model: &str,
    proxy: &str,
    language: &str,
    updated_at: i64,
) -> Result<(), String> {
    {
        let mut conn = state
            .db
            .lock()
            .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
        persist_stt_settings(&mut conn, base_url, model, proxy, language, updated_at)?;
        // Guard drops here before the async lock below — a `std` MutexGuard
        // must never be held across an `.await`.
    }

    let mut validated = state.stt_key_validated.lock().await;
    *validated = false;
    Ok(())
}

/// Validate the STT settings and write all four keys **atomically**. Returns a
/// user-facing Russian error on any invalid field; nothing is written unless
/// every field validates, and the four UPSERTs commit together (transaction —
/// invariant «multi-step writes transaction-wrapped»).
pub(crate) fn persist_stt_settings(
    conn: &mut Connection,
    base_url: &str,
    model: &str,
    proxy: &str,
    language: &str,
    updated_at: i64,
) -> Result<(), String> {
    let base_url = base_url.trim();
    validation::validate_base_url(base_url)?;

    let model = model.trim();
    if model.is_empty() {
        return Err("Укажите модель распознавания (например whisper-large-v3-turbo).".to_string());
    }

    let proxy = proxy.trim();
    if !proxy.is_empty() {
        validation::validate_proxy(proxy)?;
    }

    if !matches!(language, "ru" | "en" | "auto") {
        return Err("Недопустимый язык распознавания (ожидается ru, en или auto).".to_string());
    }

    let tx = conn
        .transaction()
        .map_err(|e| format!("Не удалось открыть транзакцию: {e}"))?;
    for (key, value) in [
        (KEY_BASE_URL, base_url),
        (KEY_MODEL, model),
        (KEY_PROXY, proxy),
        (KEY_LANGUAGE, language),
    ] {
        repository::set_setting(&tx, key, value, updated_at)
            .map_err(|e| format!("Не удалось сохранить настройки диктовки: {e}"))?;
    }
    tx.commit()
        .map_err(|e| format!("Не удалось сохранить настройки диктовки: {e}"))?;
    Ok(())
}

/// Store the STT API key in the keyring and invalidate the process-lifetime
/// "validated" flag so the next probe re-checks.
pub(crate) async fn set_stt_key_impl(state: &AppState, key: &str) -> Result<(), String> {
    keyring::set_stt_key(key).map_err(|e| e.to_string())?;
    let mut guard = state.stt_key_validated.lock().await;
    *guard = false;
    Ok(())
}

/// Delete the STT API key (idempotent — a missing key is success) and reset
/// the "validated" flag.
pub(crate) async fn delete_stt_key_impl(state: &AppState) -> Result<(), String> {
    match keyring::delete_stt_key() {
        Ok(()) | Err(KeyringError::NotFound) => {}
        Err(e) => return Err(e.to_string()),
    }
    let mut guard = state.stt_key_validated.lock().await;
    *guard = false;
    Ok(())
}

/// Whether an STT API key is currently stored. Used by the Settings UI to show
/// whether the key slot is populated.
pub(crate) fn has_stt_key_impl() -> Result<bool, String> {
    Ok(keyring::get_stt_key().map_err(|e| e.to_string())?.is_some())
}

/// Validate the configured provider. With `force = false` a prior success this
/// process lifetime short-circuits to `Ok(())` without a network call; with
/// `force = true` (the Test button) the cache is bypassed and the live chain
/// is checked. On success the flag is set so subsequent probes are free.
pub(crate) async fn test_stt_key_impl(state: &AppState, force: bool) -> Result<(), String> {
    if !force {
        let guard = state.stt_key_validated.lock().await;
        if *guard {
            return Ok(());
        }
        // Guard drops here before any keyring/network work below.
    }

    // Extract settings under a block-scoped DB lock — no lock held across IO.
    let settings = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
        get_stt_settings_impl(&conn)
            .map_err(|e| format!("Не удалось прочитать настройки диктовки: {e}"))?
    };

    let api_key = keyring::get_stt_key().map_err(|e| e.to_string())?;

    let proxy = non_empty(&settings.proxy);
    let client = build_stt_client(proxy)?;
    let provider = OpenAiCompatStt::new(client, &settings.base_url, &settings.model, api_key);

    check_provider(&provider, &settings.language).await?;

    let mut guard = state.stt_key_validated.lock().await;
    *guard = true;
    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────────

/// Build a dedicated STT HTTP client. Separate from the SaluteSpeech
/// (`AppState.http_client`) client so a proxy applies to STT traffic only
/// (kickoff D5). Uses rustls; standard public CAs (no Sber cert needed).
fn build_stt_client(proxy: Option<&str>) -> Result<Client, String> {
    let mut builder = Client::builder().use_rustls_tls();
    if let Some(raw) = proxy {
        let normalized = validation::normalize_proxy(raw)?;
        let proxy =
            Proxy::all(&normalized).map_err(|e| format!("Не удалось применить прокси: {e}"))?;
        builder = builder.proxy(proxy);
    }
    builder
        .build()
        .map_err(|e| format!("Не удалось создать HTTP-клиент: {e}"))
}

/// Two-step key check (D8): try the cheap `GET /models` probe first; if the
/// provider does not expose it (a `4xx/5xx` `Api` error or an unfamiliar body)
/// fall back to transcribing 0.5 s of silence, which exercises the entire
/// key + endpoint + proxy + transcribe path. Auth/Balance/Network failures are
/// reported immediately — retrying via transcribe would not change the answer.
/// A 429 means auth already succeeded, so it counts as "key works".
async fn check_provider(provider: &OpenAiCompatStt, language: &str) -> Result<(), String> {
    match provider.list_models().await {
        Ok(_) => Ok(()),
        Err(SttError::RateLimited(_)) => Ok(()),
        Err(
            e @ (SttError::Auth | SttError::Balance | SttError::Network(_) | SttError::TooLarge),
        ) => Err(stt_error_to_user_facing_ru(&e)),
        Err(SttError::Api(_) | SttError::InvalidResponse(_)) => {
            micro_transcription_check(provider, language).await
        }
    }
}

/// Probe the full transcribe path with a tiny silence clip. Note: this goes
/// straight to the client, deliberately bypassing the recording pipeline's
/// anti-hallucination filter (which lives in PR2+, not the client), so the
/// silence sample is never rejected before it reaches the provider.
async fn micro_transcription_check(
    provider: &OpenAiCompatStt,
    language: &str,
) -> Result<(), String> {
    let clip = wav::silence_wav_s16le_mono(PROBE_DURATION_MS, PROBE_SAMPLE_RATE);
    match provider.transcribe(clip, resolve_language(language)).await {
        Ok(_) => Ok(()),
        Err(SttError::RateLimited(_)) => Ok(()),
        Err(e) => Err(stt_error_to_user_facing_ru(&e)),
    }
}

/// Map the STT language setting to the `transcribe` argument: `auto` (or an
/// unexpected/empty value) → `None` (send no language field), otherwise the
/// pinned language code.
fn resolve_language(language: &str) -> Option<&str> {
    match language {
        "auto" | "" => None,
        other => Some(other),
    }
}

/// `None` for an empty/whitespace-only string, else the trimmed value.
fn non_empty(s: &str) -> Option<&str> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

/// Translate a structured [`SttError`] into a concrete, actionable Russian
/// sentence for the Settings toast/status. The `SttError` `Display` forms stay
/// English (tests assert on them); only this boundary produces user text.
pub(crate) fn stt_error_to_user_facing_ru(err: &SttError) -> String {
    match err {
        SttError::Auth => {
            "Ключ недействителен: провайдер отклонил авторизацию (401). Проверьте ключ.".to_string()
        }
        SttError::TooLarge => "Аудиофайл слишком большой для провайдера (413).".to_string(),
        SttError::Balance => {
            "Недостаточно средств на балансе у провайдера (402). Пополните счёт.".to_string()
        }
        SttError::RateLimited(_) => {
            "Провайдер временно ограничил частоту запросов (429). Попробуйте позже.".to_string()
        }
        SttError::Network(e) => {
            format!("Не удалось связаться с провайдером: {e}. Проверьте адрес эндпоинта и прокси.")
        }
        SttError::Api(msg) => format!("Провайдер вернул ошибку: {msg}"),
        SttError::InvalidResponse(msg) => format!("Неожиданный ответ провайдера: {msg}"),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::salute::http;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn init_mock() {
        INIT.call_once(|| {
            ::keyring::set_default_credential_builder(::keyring::mock::default_credential_builder());
        });
    }

    fn fresh_state() -> AppState {
        let client = http::build_client().expect("client builds");
        let conn = crate::db::test_connection();
        AppState::new(client, conn)
    }

    // ── settings ──

    #[test]
    fn get_stt_settings_returns_defaults_on_empty_db() {
        let conn = crate::db::test_connection();
        let s = get_stt_settings_impl(&conn).unwrap();
        assert_eq!(s.base_url, DEFAULT_BASE_URL);
        assert_eq!(s.model, DEFAULT_MODEL);
        assert_eq!(s.proxy, "");
        assert_eq!(s.language, DEFAULT_LANGUAGE);
    }

    #[test]
    fn save_then_get_round_trips() {
        let mut conn = crate::db::test_connection();
        persist_stt_settings(
            &mut conn,
            "https://api.groq.com/openai/v1",
            "whisper-large-v3",
            "user:pass@proxy.example.com:8080",
            "en",
            1_700_000_000_000,
        )
        .expect("valid settings save");

        let s = get_stt_settings_impl(&conn).unwrap();
        assert_eq!(s.base_url, "https://api.groq.com/openai/v1");
        assert_eq!(s.model, "whisper-large-v3");
        assert_eq!(s.proxy, "user:pass@proxy.example.com:8080");
        assert_eq!(s.language, "en");
    }

    #[test]
    fn save_trims_and_allows_empty_proxy() {
        let mut conn = crate::db::test_connection();
        persist_stt_settings(
            &mut conn,
            "  https://api.aitunnel.ru/v1  ",
            "  whisper-1  ",
            "   ",
            "ru",
            1,
        )
        .expect("empty proxy is allowed");
        let s = get_stt_settings_impl(&conn).unwrap();
        assert_eq!(s.base_url, "https://api.aitunnel.ru/v1");
        assert_eq!(s.model, "whisper-1");
        assert_eq!(s.proxy, "");
    }

    #[test]
    fn save_rejects_external_http_base_url() {
        let mut conn = crate::db::test_connection();
        let err = persist_stt_settings(&mut conn, "http://evil.example.com/v1", "m", "", "ru", 1)
            .unwrap_err();
        assert!(err.contains("https"), "got: {err}");
        // Nothing persisted → still defaults.
        assert_eq!(
            get_stt_settings_impl(&conn).unwrap().base_url,
            DEFAULT_BASE_URL
        );
    }

    #[test]
    fn save_rejects_empty_model() {
        let mut conn = crate::db::test_connection();
        let err = persist_stt_settings(&mut conn, "https://x/v1", "   ", "", "ru", 1).unwrap_err();
        assert!(err.contains("модель"), "got: {err}");
    }

    #[test]
    fn save_rejects_bad_proxy() {
        let mut conn = crate::db::test_connection();
        let err = persist_stt_settings(&mut conn, "https://x/v1", "m", "no-port-here", "ru", 1)
            .unwrap_err();
        assert!(err.contains("порт") || err.contains("прокси"), "got: {err}");
    }

    #[test]
    fn save_rejects_bad_language() {
        let mut conn = crate::db::test_connection();
        let err = persist_stt_settings(&mut conn, "https://x/v1", "m", "", "fr", 1).unwrap_err();
        assert!(err.contains("язык"), "got: {err}");
    }

    #[tokio::test]
    async fn save_stt_settings_impl_resets_validated_flag() {
        // Changing the endpoint/proxy/model must invalidate the process-lifetime
        // "key validated" cache, or a later force=false probe would report a
        // config that was never actually checked.
        init_mock();
        let state = fresh_state();
        *state.stt_key_validated.lock().await = true;

        save_stt_settings_impl(
            &state,
            "https://api.groq.com/openai/v1",
            "whisper-large-v3",
            "",
            "en",
            1_700_000_000_000,
        )
        .await
        .expect("valid settings save");

        assert!(
            !*state.stt_key_validated.lock().await,
            "saving settings must reset the validated flag"
        );
        // And the values actually persisted.
        let conn = state.db.lock().unwrap();
        assert_eq!(
            get_stt_settings_impl(&conn).unwrap().base_url,
            "https://api.groq.com/openai/v1"
        );
    }

    #[tokio::test]
    async fn save_stt_settings_impl_rejects_and_leaves_flag_untouched() {
        // A validation failure must not write anything AND must not flip the
        // flag (nothing was validated).
        init_mock();
        let state = fresh_state();
        *state.stt_key_validated.lock().await = true;

        let err = save_stt_settings_impl(&state, "http://evil.example.com/v1", "m", "", "ru", 1)
            .await
            .unwrap_err();
        assert!(err.contains("https"), "got: {err}");
        assert!(
            *state.stt_key_validated.lock().await,
            "a rejected save must leave the flag as it was"
        );
    }

    // ── helpers ──

    #[test]
    fn resolve_language_maps_auto_and_empty_to_none() {
        assert_eq!(resolve_language("auto"), None);
        assert_eq!(resolve_language(""), None);
        assert_eq!(resolve_language("ru"), Some("ru"));
        assert_eq!(resolve_language("en"), Some("en"));
    }

    #[test]
    fn non_empty_trims() {
        assert_eq!(non_empty("  "), None);
        assert_eq!(non_empty(""), None);
        assert_eq!(non_empty("  x "), Some("x"));
    }

    #[test]
    fn build_stt_client_none_proxy_ok() {
        assert!(build_stt_client(None).is_ok());
    }

    #[test]
    fn build_stt_client_bad_proxy_errors() {
        let err = build_stt_client(Some("missing-port")).unwrap_err();
        assert!(err.contains("порт") || err.contains("прокси"), "got: {err}");
    }

    #[test]
    fn stt_error_to_user_facing_ru_covers_variants() {
        assert!(stt_error_to_user_facing_ru(&SttError::Auth).contains("401"));
        assert!(stt_error_to_user_facing_ru(&SttError::Balance).contains("402"));
        assert!(stt_error_to_user_facing_ru(&SttError::RateLimited(None)).contains("429"));
        assert!(stt_error_to_user_facing_ru(&SttError::TooLarge).contains("413"));
        assert!(stt_error_to_user_facing_ru(&SttError::Api("boom".into())).contains("boom"));
        assert!(
            stt_error_to_user_facing_ru(&SttError::InvalidResponse("x".into()))
                .contains("Неожиданный")
        );
    }

    // ── key management ──

    #[tokio::test]
    async fn set_stt_key_resets_validated_flag() {
        init_mock();
        let state = fresh_state();
        *state.stt_key_validated.lock().await = true;

        set_stt_key_impl(&state, "some-key").await.expect("save ok");
        assert!(
            !*state.stt_key_validated.lock().await,
            "saving a key must reset the validated flag"
        );
    }

    #[tokio::test]
    async fn set_stt_key_rejects_empty() {
        init_mock();
        let state = fresh_state();
        let err = set_stt_key_impl(&state, "   ").await.unwrap_err();
        assert!(
            err.contains("empty") || err.contains("whitespace"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn delete_stt_key_idempotent_and_resets_flag() {
        init_mock();
        let state = fresh_state();
        *state.stt_key_validated.lock().await = true;

        // Mock keyring starts empty → NotFound branch → mapped to Ok.
        delete_stt_key_impl(&state)
            .await
            .expect("delete idempotent");
        assert!(!*state.stt_key_validated.lock().await);
    }

    // ── test_stt_key: cache-first + live provider (mockito) ──

    #[tokio::test]
    async fn test_stt_key_cache_first_short_circuits() {
        init_mock();
        let state = fresh_state();
        *state.stt_key_validated.lock().await = true;

        // force=false with the flag set must return Ok without any network:
        // the default base_url (real AITunnel) is never contacted.
        test_stt_key_impl(&state, false)
            .await
            .expect("cache hit returns Ok without network");
    }

    #[tokio::test]
    async fn test_stt_key_success_against_models_endpoint() {
        init_mock();
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/models")
            .with_status(200)
            .with_body(r#"{"data":[{"id":"whisper-1"}]}"#)
            .create_async()
            .await;

        let state = fresh_state();
        {
            let mut conn = state.db.lock().unwrap();
            // base_url = mock root so models_url resolves to `{root}/models`.
            persist_stt_settings(&mut conn, &server.url(), "whisper-1", "", "ru", 1)
                .expect("seed settings");
        }

        test_stt_key_impl(&state, true)
            .await
            .expect("live /models 200 validates");
        assert!(
            *state.stt_key_validated.lock().await,
            "success must set the validated flag"
        );
    }

    #[tokio::test]
    async fn test_stt_key_auth_error_is_russian_and_flag_stays_false() {
        init_mock();
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/models")
            .with_status(401)
            .with_body("nope")
            .create_async()
            .await;

        let state = fresh_state();
        {
            let mut conn = state.db.lock().unwrap();
            persist_stt_settings(&mut conn, &server.url(), "whisper-1", "", "ru", 1).unwrap();
        }

        let err = test_stt_key_impl(&state, true).await.unwrap_err();
        assert!(
            err.contains("401"),
            "should surface auth error in Russian: {err}"
        );
        assert!(
            !*state.stt_key_validated.lock().await,
            "a failed check must not set the validated flag"
        );
    }

    #[tokio::test]
    async fn test_stt_key_falls_back_to_transcription_when_models_missing() {
        init_mock();
        let mut server = mockito::Server::new_async().await;
        // /models absent → 404; transcribe path answers 200.
        let _models = server
            .mock("GET", "/models")
            .with_status(404)
            .with_body("not found")
            .create_async()
            .await;
        let _tx = server
            .mock("POST", "/audio/transcriptions")
            .with_status(200)
            .with_body(r#"{"text":""}"#)
            .create_async()
            .await;

        let state = fresh_state();
        {
            let mut conn = state.db.lock().unwrap();
            persist_stt_settings(&mut conn, &server.url(), "whisper-1", "", "ru", 1).unwrap();
        }

        test_stt_key_impl(&state, true)
            .await
            .expect("404 on /models falls back to a successful micro-transcription");
        assert!(*state.stt_key_validated.lock().await);
    }
}
