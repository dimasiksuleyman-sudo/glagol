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

use cpal::traits::{DeviceTrait, HostTrait};
use reqwest::{Client, Proxy};
use rusqlite::Connection;
use serde::Serialize;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::db::repository;
use crate::db::repository::Dictation;
use crate::dictation::insert::{
    InsertionMode, INSERTION_MODE_CLIPBOARD_ONLY, INSERTION_MODE_PASTE,
};
use crate::dictation::RecorderError;
use crate::secrets::keyring::{self, KeyringError};
use crate::state::AppState;
use crate::stt::openai_compat::OpenAiCompatStt;
use crate::stt::{validation, wav, SttError};

// ── Settings keys + defaults ───────────────────────────────────────────

const KEY_BASE_URL: &str = "stt_base_url";
const KEY_MODEL: &str = "stt_model";
const KEY_PROXY: &str = "stt_proxy";
const KEY_LANGUAGE: &str = "stt_language";
/// `app_settings` key for the auto-insertion mode (Sprint 6 PR4, D12). Written by
/// the PR5 radio button; read here. Absent → the [`InsertionMode::Paste`] default.
const KEY_INSERTION_MODE: &str = "stt_insertion_mode";

// ── Dictation-page settings keys + defaults (Sprint 6 PR5a, D3) ─────────
//
// Each key is read with its default in Rust (never INSERTed by the migration)
// so a default can change with a code patch, and an absent key resolves to the
// default rather than panicking (D3). An unknown/corrupt value is a **loud**
// fallback, mirroring the D12 insertion-mode reader.

/// Push-to-talk hotkey (D7). global-shortcut accelerator string; the default is
/// `Control+Shift+Space` on Windows/Linux, `Cmd+Shift+Space` on macOS.
const KEY_HOTKEY: &str = "dictation_hotkey";
/// Pinned input device **name** (D6). Empty = system default.
const KEY_DEVICE: &str = "dictation_device";
/// History opt-in toggle (D4). Default **off** — when off, no transcript is
/// written to `dictations` at all.
const KEY_HISTORY_ENABLED: &str = "dictation_history_enabled";
/// STT provider preset name (D3). Informational label for the Settings UI.
const KEY_PROVIDER: &str = "stt_provider";

/// Default push-to-talk hotkey (D3/D7). `CmdOrCtrl` resolves to `Control` on
/// Windows/Linux and `Cmd` on macOS via global-shortcut's parser.
pub(crate) const DEFAULT_HOTKEY: &str = "CmdOrCtrl+Shift+Space";
/// Default STT provider preset (D3).
const DEFAULT_PROVIDER: &str = "aitunnel";
/// Default history state: **off** (D4/Q3).
const DEFAULT_HISTORY_ENABLED: bool = false;
/// Persisted representation of an enabled/disabled boolean setting.
const BOOL_TRUE: &str = "true";
const BOOL_FALSE: &str = "false";

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

/// Dictation-page settings returned to the (PR5b) Settings UI (D3). All fields
/// carry their in-code defaults when the underlying key is unset. `Serialize`
/// only — it is a command return type, never an input (an input would want the
/// per-field setter instead).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DictationSettings {
    /// global-shortcut accelerator string (D7).
    pub hotkey: String,
    /// Pinned device name; empty = system default (D6).
    pub device: String,
    /// History opt-in (D4).
    pub history_enabled: bool,
    /// Provider preset name (D3).
    pub provider: String,
    /// Recognition model id (shared with the STT settings block).
    pub model: String,
    /// `"paste"` | `"clipboard_only"` — the effective auto-insertion mode (D12).
    pub insertion_mode: String,
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

/// Read the persisted auto-insertion mode (D12). An absent key defaults to
/// [`InsertionMode::Paste`]; a present-but-unknown value is a loud fallback to
/// [`InsertionMode::ClipboardOnly`] (handled inside
/// [`crate::dictation::insert::parse_insertion_mode`]) — never a silent guess at
/// `Paste`.
pub(crate) fn read_insertion_mode(conn: &Connection) -> rusqlite::Result<InsertionMode> {
    Ok(match repository::get_setting(conn, KEY_INSERTION_MODE)? {
        None => InsertionMode::Paste,
        Some(raw) => crate::dictation::insert::parse_insertion_mode(&raw),
    })
}

/// Read the history opt-in toggle (D4). Absent → [`DEFAULT_HISTORY_ENABLED`]
/// (off). An unknown/corrupt value is a **loud** fallback to off, never a silent
/// guess — mirrors the D12 insertion-mode reader. Off is the safe default: it
/// keeps transcripts off disk.
pub(crate) fn read_history_enabled(conn: &Connection) -> rusqlite::Result<bool> {
    Ok(match repository::get_setting(conn, KEY_HISTORY_ENABLED)? {
        None => DEFAULT_HISTORY_ENABLED,
        Some(raw) => match raw.as_str() {
            BOOL_TRUE => true,
            BOOL_FALSE => false,
            other => {
                tracing::error!(
                    value = other,
                    "unknown dictation_history_enabled; falling back to off"
                );
                DEFAULT_HISTORY_ENABLED
            }
        },
    })
}

/// Read the pinned input device name (D6). An absent or whitespace-only value
/// means "system default" → `None`.
pub(crate) fn read_device_setting(conn: &Connection) -> rusqlite::Result<Option<String>> {
    Ok(repository::get_setting(conn, KEY_DEVICE)?
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty()))
}

/// Persisted string form of an [`InsertionMode`] for the settings DTO.
fn insertion_mode_str(mode: InsertionMode) -> &'static str {
    match mode {
        InsertionMode::Paste => INSERTION_MODE_PASTE,
        InsertionMode::ClipboardOnly => INSERTION_MODE_CLIPBOARD_ONLY,
    }
}

/// Read every dictation-page setting, substituting defaults for unset keys (D3).
pub(crate) fn get_dictation_settings_impl(
    conn: &Connection,
) -> rusqlite::Result<DictationSettings> {
    Ok(DictationSettings {
        hotkey: repository::get_setting(conn, KEY_HOTKEY)?
            .unwrap_or_else(|| DEFAULT_HOTKEY.to_string()),
        device: repository::get_setting(conn, KEY_DEVICE)?.unwrap_or_default(),
        history_enabled: read_history_enabled(conn)?,
        provider: repository::get_setting(conn, KEY_PROVIDER)?
            .unwrap_or_else(|| DEFAULT_PROVIDER.to_string()),
        model: repository::get_setting(conn, KEY_MODEL)?
            .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
        insertion_mode: insertion_mode_str(read_insertion_mode(conn)?).to_string(),
    })
}

/// Validate + persist a single dictation-page setting (D3). The `name` is
/// whitelisted (an unknown key is rejected, not written), the value is validated
/// per key, and the hotkey is deliberately **not** settable here — it goes
/// through [`set_dictation_hotkey_impl`] so the live unregister/register with
/// rollback (D7) always runs. Returns a user-facing Russian error on any invalid
/// input; nothing is written unless it validates.
pub(crate) fn set_dictation_setting_impl(
    conn: &Connection,
    name: &str,
    value: &str,
    updated_at: i64,
) -> Result<(), String> {
    let value = value.trim();
    match name {
        KEY_HISTORY_ENABLED => {
            if value != BOOL_TRUE && value != BOOL_FALSE {
                return Err(
                    "Недопустимое значение переключателя истории (ожидается true или false)."
                        .to_string(),
                );
            }
        }
        KEY_INSERTION_MODE => {
            if value != INSERTION_MODE_PASTE && value != INSERTION_MODE_CLIPBOARD_ONLY {
                return Err(
                    "Недопустимый режим вставки (ожидается paste или clipboard_only).".to_string(),
                );
            }
        }
        KEY_MODEL => {
            if value.is_empty() {
                return Err(
                    "Укажите модель распознавания (например whisper-large-v3-turbo).".to_string(),
                );
            }
        }
        KEY_PROVIDER => {
            if value.is_empty() {
                return Err("Укажите провайдера распознавания.".to_string());
            }
        }
        // Device name is free-form (empty = system default); no validation.
        KEY_DEVICE => {}
        KEY_HOTKEY => {
            return Err(
                "Хоткей меняется отдельной командой (set_dictation_hotkey) с проверкой конфликта."
                    .to_string(),
            );
        }
        other => return Err(format!("Неизвестная настройка диктовки: {other}")),
    }

    repository::set_setting(conn, name, value, updated_at)
        .map_err(|e| format!("Не удалось сохранить настройку диктовки: {e}"))?;
    Ok(())
}

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
pub(crate) fn build_stt_client(proxy: Option<&str>) -> Result<Client, String> {
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

// ── Audio input devices (Sprint 6 PR2) ─────────────────────────────────

/// List the names of the system's audio input devices for the (future) device
/// picker. Enumeration needs no capture stream, so this runs directly on the
/// host without involving the recorder thread. `None`-or-missing selection in
/// settings means "system default"; this list populates the UI that lets the
/// user pin a specific device (the picker itself lands in PR5).
#[tauri::command]
pub async fn list_audio_input_devices() -> Result<Vec<String>, String> {
    list_audio_input_devices_impl().map_err(|e| recorder_error_to_user_facing_ru(&e))
}

/// Enumerate input-device names. Returns them in host order; an empty machine
/// yields an empty list rather than an error (the UI shows "нет устройств").
pub(crate) fn list_audio_input_devices_impl() -> Result<Vec<String>, RecorderError> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|e| RecorderError::UnsupportedConfig(e.to_string()))?;
    // cpal 0.17 deprecated `Device::name()` in favour of `description()`;
    // devices whose description can't be read are skipped rather than failing.
    Ok(devices
        .filter_map(|d| d.description().ok().map(|desc| desc.name().to_string()))
        .collect())
}

/// Translate a [`RecorderError`] into a concrete, actionable Russian sentence
/// for the UI (the `Display` forms stay English so tests assert on them —
/// mirrors [`stt_error_to_user_facing_ru`]).
pub(crate) fn recorder_error_to_user_facing_ru(err: &RecorderError) -> String {
    match err {
        RecorderError::NoDevice => {
            "Микрофон не найден. Подключите устройство ввода звука.".to_string()
        }
        RecorderError::PermissionDenied => {
            "Нет доступа к микрофону. Проверьте Параметры → Конфиденциальность → Микрофон."
                .to_string()
        }
        RecorderError::UnsupportedConfig(details) => {
            format!("Микрофон не поддерживает нужный формат записи: {details}.")
        }
        RecorderError::BuildStream(details) => {
            format!("Не удалось запустить запись с микрофона: {details}.")
        }
        RecorderError::DeviceLost(details) => {
            format!("Связь с микрофоном потеряна: {details}.")
        }
        RecorderError::Busy => "Запись уже идёт.".to_string(),
    }
}

// ── Dictation-page commands (Sprint 6 PR5a) ────────────────────────────

#[tauri::command]
pub async fn get_dictation_settings(
    state: tauri::State<'_, AppState>,
) -> Result<DictationSettings, String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
    get_dictation_settings_impl(&conn)
        .map_err(|e| format!("Не удалось прочитать настройки диктовки: {e}"))
}

#[tauri::command]
pub async fn set_dictation_setting(
    state: tauri::State<'_, AppState>,
    name: String,
    value: String,
) -> Result<(), String> {
    let updated_at = chrono::Utc::now().timestamp_millis();
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
    set_dictation_setting_impl(&conn, &name, &value, updated_at)
}

#[tauri::command]
pub async fn list_dictations(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<Dictation>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
    list_dictations_impl(&conn, limit)
        .map_err(|e| format!("Не удалось прочитать историю диктовки: {e}"))
}

#[tauri::command]
pub async fn clear_dictation_history(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
    repository::clear_dictations(&conn)
        .map(|_| ())
        .map_err(|e| format!("Не удалось очистить историю диктовки: {e}"))
}

#[tauri::command]
pub async fn get_recognitions_minutes(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
    get_recognitions_minutes_impl(&conn)
        .map_err(|e| format!("Не удалось прочитать счётчик диктовки: {e}"))
}

#[tauri::command]
pub async fn set_dictation_hotkey(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    hotkey: String,
) -> Result<(), String> {
    let updated_at = chrono::Utc::now().timestamp_millis();
    let old = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
        repository::get_setting(&conn, KEY_HOTKEY)
            .map_err(|e| format!("Не удалось прочитать текущий хоткей: {e}"))?
            .unwrap_or_else(|| DEFAULT_HOTKEY.to_string())
        // Guard drops before the register/unregister below.
    };

    let registrar = AppHotkeyRegistrar { app: &app };
    set_dictation_hotkey_impl(&state, &registrar, &old, &hotkey, updated_at)
}

/// Gating (D4): when history is **off**, `list_dictations` returns empty even if
/// stale rows survive from a period when it was on — "off" means the UI never
/// surfaces transcripts. When on, returns rows newest-first, capped by `limit`.
pub(crate) fn list_dictations_impl(
    conn: &Connection,
    limit: Option<i64>,
) -> rusqlite::Result<Vec<Dictation>> {
    if !read_history_enabled(conn)? {
        return Ok(Vec::new());
    }
    repository::list_dictations(conn, limit)
}

/// The lifetime «Надиктовано» figure in whole minutes (D4). Reads the always-on
/// `recognitions_seconds` ledger (independent of the history toggle) and floors
/// to minutes.
pub(crate) fn get_recognitions_minutes_impl(conn: &Connection) -> rusqlite::Result<u64> {
    let seconds = repository::sum_recognition_seconds(conn)?;
    Ok((seconds.max(0) as u64) / 60)
}

// ── Hotkey re-registration with rollback (D7) ──────────────────────────

/// The register/unregister seam so the D7 rollback is testable without a Tauri
/// runtime. The real implementation ([`AppHotkeyRegistrar`]) forwards to the
/// global-shortcut plugin; a fake in tests can make `register` fail to exercise
/// the rollback branch.
pub(crate) trait HotkeyRegistrar {
    fn register(&self, shortcut: &Shortcut) -> Result<(), String>;
    fn unregister(&self, shortcut: &Shortcut) -> Result<(), String>;
}

/// Production registrar backed by the global-shortcut plugin.
struct AppHotkeyRegistrar<'a> {
    app: &'a tauri::AppHandle,
}

impl HotkeyRegistrar for AppHotkeyRegistrar<'_> {
    fn register(&self, shortcut: &Shortcut) -> Result<(), String> {
        self.app
            .global_shortcut()
            .register(*shortcut)
            .map_err(|e| e.to_string())
    }
    fn unregister(&self, shortcut: &Shortcut) -> Result<(), String> {
        self.app
            .global_shortcut()
            .unregister(*shortcut)
            .map_err(|e| e.to_string())
    }
}

/// The hotkey to register at startup (D7): the saved `dictation_hotkey` if it
/// parses, else the default. Encapsulates the key + default so `lib.rs` need not
/// know either. A DB read error or an unparseable saved value both fall through
/// to the default rather than leaving dictation without a hotkey.
pub(crate) fn startup_hotkey(conn: &Connection) -> Shortcut {
    repository::get_setting(conn, KEY_HOTKEY)
        .ok()
        .flatten()
        .and_then(|s| validate_hotkey(&s).ok())
        .unwrap_or_else(|| validate_hotkey(DEFAULT_HOTKEY).expect("default hotkey constant parses"))
}

/// Validate a hotkey accelerator string (D7), returning the parsed [`Shortcut`].
/// Pure — no Tauri runtime — so format validation is unit-testable. A parse
/// failure becomes a user-facing Russian error.
pub(crate) fn validate_hotkey(raw: &str) -> Result<Shortcut, String> {
    raw.trim().parse::<Shortcut>().map_err(|_| {
        "Некорректный хоткей. Пример: Ctrl+Shift+Space (модификатор + клавиша).".to_string()
    })
}

/// Swap the live push-to-talk hotkey with rollback (D7): unregister `old`,
/// register `new`; if registering `new` fails (another app owns the combo),
/// **re-register `old`** so the user is never left with no working hotkey, and
/// return an error. The unregister of `old` is best-effort — if the old hotkey
/// was never registered (e.g. startup registration lost the race), failing to
/// remove it must not block setting a working new one.
pub(crate) fn swap_hotkey<R: HotkeyRegistrar>(
    registrar: &R,
    old: &Shortcut,
    new: &Shortcut,
) -> Result<(), String> {
    if let Err(e) = registrar.unregister(old) {
        tracing::warn!(error = %e, "unregistering old dictation hotkey failed (continuing)");
    }
    if let Err(e) = registrar.register(new) {
        // Roll back to the working hotkey before reporting the conflict.
        if let Err(re) = registrar.register(old) {
            tracing::error!(error = %re, "failed to re-register the previous dictation hotkey after a conflict");
        }
        return Err(format!(
            "Не удалось назначить хоткей — возможно, он занят другим приложением ({e}). \
             Оставлен прежний хоткей."
        ));
    }
    Ok(())
}

/// Validate the new hotkey, swap it live with rollback (D7), and persist it only
/// on success. Generic over the registrar seam so the whole flow — including the
/// rollback branch — is unit-testable.
pub(crate) fn set_dictation_hotkey_impl<R: HotkeyRegistrar>(
    state: &AppState,
    registrar: &R,
    old_raw: &str,
    new_raw: &str,
    updated_at: i64,
) -> Result<(), String> {
    let new = validate_hotkey(new_raw)?;
    // The stored/old hotkey should be valid, but fall back to the default rather
    // than aborting if a hand-edited DB holds garbage — we still want to install
    // the new one.
    let old = validate_hotkey(old_raw).unwrap_or_else(|_| {
        validate_hotkey(DEFAULT_HOTKEY).expect("default hotkey constant must parse")
    });

    swap_hotkey(registrar, &old, &new)?;

    let conn = state
        .db
        .lock()
        .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
    repository::set_setting(&conn, KEY_HOTKEY, new_raw.trim(), updated_at)
        .map_err(|e| format!("Не удалось сохранить хоткей: {e}"))?;
    Ok(())
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
        AppState::new(
            client,
            conn,
            crate::dictation::RecorderHandle::disconnected(),
        )
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

    // ── dictation-page settings (D3) ──

    #[test]
    fn get_dictation_settings_returns_defaults_on_empty_db() {
        let conn = crate::db::test_connection();
        let s = get_dictation_settings_impl(&conn).unwrap();
        assert_eq!(s.hotkey, DEFAULT_HOTKEY);
        assert_eq!(s.device, "");
        assert!(!s.history_enabled, "history default is off (D4)");
        assert_eq!(s.provider, DEFAULT_PROVIDER);
        assert_eq!(s.model, DEFAULT_MODEL);
        assert_eq!(s.insertion_mode, INSERTION_MODE_PASTE);
    }

    #[test]
    fn read_history_enabled_default_off_reads_values_and_loud_fallback() {
        let conn = crate::db::test_connection();
        // Absent → off (D4).
        assert!(!read_history_enabled(&conn).unwrap());
        // Explicit values round-trip.
        repository::set_setting(&conn, KEY_HISTORY_ENABLED, "true", 1).unwrap();
        assert!(read_history_enabled(&conn).unwrap());
        repository::set_setting(&conn, KEY_HISTORY_ENABLED, "false", 2).unwrap();
        assert!(!read_history_enabled(&conn).unwrap());
        // Garbage → loud fallback to off, never a silent guess at on.
        repository::set_setting(&conn, KEY_HISTORY_ENABLED, "yes", 3).unwrap();
        assert!(!read_history_enabled(&conn).unwrap());
    }

    #[test]
    fn read_device_setting_maps_empty_to_none() {
        let conn = crate::db::test_connection();
        assert_eq!(read_device_setting(&conn).unwrap(), None);
        repository::set_setting(&conn, KEY_DEVICE, "  ", 1).unwrap();
        assert_eq!(read_device_setting(&conn).unwrap(), None);
        repository::set_setting(&conn, KEY_DEVICE, "USB Mic", 2).unwrap();
        assert_eq!(
            read_device_setting(&conn).unwrap(),
            Some("USB Mic".to_string())
        );
    }

    #[test]
    fn set_dictation_setting_validates_and_persists() {
        let conn = crate::db::test_connection();
        // history_enabled: only true/false.
        set_dictation_setting_impl(&conn, KEY_HISTORY_ENABLED, "true", 1).unwrap();
        assert!(read_history_enabled(&conn).unwrap());
        assert!(set_dictation_setting_impl(&conn, KEY_HISTORY_ENABLED, "maybe", 2).is_err());

        // insertion_mode: only paste/clipboard_only.
        set_dictation_setting_impl(&conn, KEY_INSERTION_MODE, "clipboard_only", 3).unwrap();
        assert!(set_dictation_setting_impl(&conn, KEY_INSERTION_MODE, "type", 4).is_err());

        // model: non-empty.
        assert!(set_dictation_setting_impl(&conn, KEY_MODEL, "   ", 5).is_err());
        set_dictation_setting_impl(&conn, KEY_MODEL, "whisper-1", 6).unwrap();

        // device: free-form, trimmed.
        set_dictation_setting_impl(&conn, KEY_DEVICE, "  Line In  ", 7).unwrap();
        assert_eq!(
            read_device_setting(&conn).unwrap(),
            Some("Line In".to_string())
        );
    }

    #[test]
    fn set_dictation_setting_rejects_hotkey_and_unknown_keys() {
        let conn = crate::db::test_connection();
        // Hotkey must go through the dedicated command (D7 rollback), never here.
        let err = set_dictation_setting_impl(&conn, KEY_HOTKEY, "Ctrl+A", 1).unwrap_err();
        assert!(err.contains("set_dictation_hotkey"), "got: {err}");
        // Arbitrary key names are rejected, not silently written.
        assert!(set_dictation_setting_impl(&conn, "evil_key", "x", 2).is_err());
    }

    #[test]
    fn list_dictations_impl_gated_by_history_toggle() {
        let mut conn = crate::db::test_connection();
        // Seed a row (simulating a period when history was on).
        repository::insert_dictation(
            &mut conn,
            &repository::NewDictation {
                created_at: 1_000,
                duration_ms: 500,
                text: "привет".to_string(),
                status: "clipboard".to_string(),
                error_message: None,
            },
        )
        .unwrap();

        // History off (default) → the list surfaces nothing even though a row exists.
        assert!(list_dictations_impl(&conn, None).unwrap().is_empty());

        // History on → the row appears.
        repository::set_setting(&conn, KEY_HISTORY_ENABLED, "true", 2).unwrap();
        let rows = list_dictations_impl(&conn, None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "привет");
    }

    #[test]
    fn get_recognitions_minutes_impl_floors_seconds() {
        let conn = crate::db::test_connection();
        assert_eq!(get_recognitions_minutes_impl(&conn).unwrap(), 0);
        // 130 s across two months → 2 minutes (floored).
        repository::record_recognition_usage(&conn, "2026-05", 70, 1).unwrap();
        repository::record_recognition_usage(&conn, "2026-06", 60, 2).unwrap();
        assert_eq!(get_recognitions_minutes_impl(&conn).unwrap(), 2);
    }

    // ── hotkey re-registration with rollback (D7) ──

    /// A fake registrar recording every call; `register` fails when its target
    /// is in `fail_on` so the rollback branch can be driven deterministically.
    #[derive(Default)]
    struct FakeRegistrar {
        registered: std::sync::Mutex<Vec<String>>,
        unregistered: std::sync::Mutex<Vec<String>>,
        fail_register_for: Option<String>,
    }

    impl HotkeyRegistrar for FakeRegistrar {
        fn register(&self, shortcut: &Shortcut) -> Result<(), String> {
            let name = format!("{shortcut:?}");
            if self.fail_register_for.as_deref() == Some(name.as_str()) {
                return Err("occupied".to_string());
            }
            self.registered.lock().unwrap().push(name);
            Ok(())
        }
        fn unregister(&self, shortcut: &Shortcut) -> Result<(), String> {
            self.unregistered
                .lock()
                .unwrap()
                .push(format!("{shortcut:?}"));
            Ok(())
        }
    }

    #[test]
    fn startup_hotkey_prefers_saved_then_falls_back_to_default() {
        let conn = crate::db::test_connection();
        let default = validate_hotkey(DEFAULT_HOTKEY).unwrap();
        // Absent → default.
        assert_eq!(startup_hotkey(&conn), default);
        // Saved + valid → saved.
        repository::set_setting(&conn, KEY_HOTKEY, "Alt+Shift+D", 1).unwrap();
        assert_eq!(
            startup_hotkey(&conn),
            validate_hotkey("Alt+Shift+D").unwrap()
        );
        // Saved + garbage → default (never leaves dictation without a hotkey).
        repository::set_setting(&conn, KEY_HOTKEY, "!!broken!!", 2).unwrap();
        assert_eq!(startup_hotkey(&conn), default);
    }

    #[test]
    fn validate_hotkey_accepts_default_and_rejects_garbage() {
        assert!(validate_hotkey(DEFAULT_HOTKEY).is_ok());
        assert!(validate_hotkey("Ctrl+Shift+Space").is_ok());
        let err = validate_hotkey("not a hotkey!!").unwrap_err();
        assert!(err.contains("хоткей"), "got: {err}");
        assert!(validate_hotkey("").is_err());
    }

    #[test]
    fn swap_hotkey_success_registers_new_and_unregisters_old() {
        let old = validate_hotkey("Ctrl+Shift+Space").unwrap();
        let new = validate_hotkey("Alt+Shift+D").unwrap();
        let reg = FakeRegistrar::default();
        swap_hotkey(&reg, &old, &new).expect("swap succeeds");
        assert_eq!(reg.registered.lock().unwrap().len(), 1);
        assert_eq!(reg.unregistered.lock().unwrap().len(), 1);
    }

    #[test]
    fn swap_hotkey_rolls_back_when_new_is_occupied() {
        // D7 negative cycle: registering the new hotkey fails (occupied) → the
        // OLD hotkey must be re-registered so the user is never left with none.
        let old = validate_hotkey("Ctrl+Shift+Space").unwrap();
        let new = validate_hotkey("Alt+Shift+D").unwrap();
        let reg = FakeRegistrar {
            fail_register_for: Some(format!("{new:?}")),
            ..Default::default()
        };
        let err = swap_hotkey(&reg, &old, &new).unwrap_err();
        assert!(err.contains("занят"), "conflict message expected: {err}");
        // The rollback re-registered the OLD hotkey (the only successful register).
        let registered = reg.registered.lock().unwrap();
        assert_eq!(registered.len(), 1);
        assert_eq!(registered[0], format!("{old:?}"));
    }

    #[tokio::test]
    async fn set_dictation_hotkey_impl_persists_on_success() {
        init_mock();
        let state = fresh_state();
        let reg = FakeRegistrar::default();
        set_dictation_hotkey_impl(&state, &reg, DEFAULT_HOTKEY, "Alt+Shift+D", 1)
            .expect("valid hotkey installs");
        let conn = state.db.lock().unwrap();
        assert_eq!(
            repository::get_setting(&conn, KEY_HOTKEY).unwrap(),
            Some("Alt+Shift+D".to_string())
        );
    }

    #[tokio::test]
    async fn set_dictation_hotkey_impl_does_not_persist_on_conflict() {
        // D7: a conflict must leave the stored hotkey unchanged.
        init_mock();
        let state = fresh_state();
        let new = validate_hotkey("Alt+Shift+D").unwrap();
        let reg = FakeRegistrar {
            fail_register_for: Some(format!("{new:?}")),
            ..Default::default()
        };
        let err =
            set_dictation_hotkey_impl(&state, &reg, DEFAULT_HOTKEY, "Alt+Shift+D", 1).unwrap_err();
        assert!(err.contains("занят"), "got: {err}");
        let conn = state.db.lock().unwrap();
        assert_eq!(
            repository::get_setting(&conn, KEY_HOTKEY).unwrap(),
            None,
            "a failed swap must not write the hotkey setting"
        );
    }

    #[test]
    fn set_dictation_setting_rejects_bad_hotkey_stays_out_of_this_path() {
        // Guard: the generic setter never touches the hotkey key.
        let conn = crate::db::test_connection();
        assert!(set_dictation_setting_impl(&conn, KEY_HOTKEY, DEFAULT_HOTKEY, 1).is_err());
        assert_eq!(repository::get_setting(&conn, KEY_HOTKEY).unwrap(), None);
    }

    // ── insertion mode (D12) ──

    #[test]
    fn insertion_mode_defaults_to_paste_on_empty_db() {
        // D12: an absent key means the auto-paste default.
        let conn = crate::db::test_connection();
        assert_eq!(read_insertion_mode(&conn).unwrap(), InsertionMode::Paste);
    }

    #[test]
    fn insertion_mode_reads_persisted_values() {
        let conn = crate::db::test_connection();
        repository::set_setting(&conn, KEY_INSERTION_MODE, "clipboard_only", 1).unwrap();
        assert_eq!(
            read_insertion_mode(&conn).unwrap(),
            InsertionMode::ClipboardOnly
        );
        repository::set_setting(&conn, KEY_INSERTION_MODE, "paste", 2).unwrap();
        assert_eq!(read_insertion_mode(&conn).unwrap(), InsertionMode::Paste);
    }

    #[test]
    fn insertion_mode_unknown_value_falls_back_to_clipboard_only() {
        // D12: a garbage value must NOT silently become Paste.
        let conn = crate::db::test_connection();
        repository::set_setting(&conn, KEY_INSERTION_MODE, "type", 1).unwrap();
        assert_eq!(
            read_insertion_mode(&conn).unwrap(),
            InsertionMode::ClipboardOnly
        );
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

    // ── audio input devices ──

    #[test]
    fn list_audio_input_devices_impl_does_not_panic() {
        // On a headless CI box this may return Ok(vec![]) or an enumeration
        // error; either is fine. The contract under test is "never panics".
        let _ = list_audio_input_devices_impl();
    }

    #[test]
    fn recorder_error_ru_is_actionable_for_every_variant() {
        assert!(recorder_error_to_user_facing_ru(&RecorderError::NoDevice).contains("Микрофон"));
        assert!(
            recorder_error_to_user_facing_ru(&RecorderError::PermissionDenied)
                .contains("Конфиденциальность")
        );
        assert!(
            recorder_error_to_user_facing_ru(&RecorderError::UnsupportedConfig("F64".to_string()))
                .contains("F64")
        );
        assert!(
            recorder_error_to_user_facing_ru(&RecorderError::BuildStream("x".to_string()))
                .contains("запись")
        );
        assert!(
            recorder_error_to_user_facing_ru(&RecorderError::DeviceLost("y".to_string()))
                .contains("потеряна")
        );
        assert!(recorder_error_to_user_facing_ru(&RecorderError::Busy).contains("уже идёт"));
    }
}
