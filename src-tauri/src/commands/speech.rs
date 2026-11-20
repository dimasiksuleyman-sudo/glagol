//! Speech profiles: on-device, office server and cloud. Remote credentials are
//! separated by mode and bound to the endpoint they were saved for.
use super::speech_model_check::{self, SavedModelStatus};
use crate::{
    db::repository,
    secrets::keyring,
    state::AppState,
    stt::{
        local::{LocalModels, LocalProvider},
        openai_compat::{ModelProbeError, OpenAiCompatStt},
        validation, SttBackend, SttError,
    },
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Emitter, Manager};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Local,
    Server,
    Cloud,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Server => "server",
            Self::Cloud => "cloud",
        }
    }
    pub(super) fn prefix(self) -> &'static str {
        if self == Self::Server {
            "stt_server"
        } else {
            "stt"
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub mode: Mode,
    pub base_url: String,
    pub model: String,
    pub proxy: String,
    pub language: String,
}

#[derive(Serialize)]
pub struct Settings {
    pub profile: Profile,
    pub active_mode: Mode,
    pub key_stored: bool,
    pub model_status: SavedModelStatus,
}

pub fn active_mode(conn: &Connection) -> Result<Mode, String> {
    let stored = repository::get_setting(conn, "stt_mode").map_err(|e| e.to_string())?;
    Ok(match stored.as_deref() {
        Some("local") => Mode::Local,
        Some("server") => Mode::Server,
        // Legacy endpoints remain in the cloud profile until explicitly saved as a server.
        _ => Mode::Cloud,
    })
}

pub fn read_profile(conn: &Connection, mode: Option<Mode>) -> Result<Profile, String> {
    let mode = mode.unwrap_or(active_mode(conn)?);
    let read = |key: &str, default: &str| -> Result<String, String> {
        Ok(
            repository::get_setting(conn, &format!("{}_{}", mode.prefix(), key))
                .map_err(|e| e.to_string())?
                .unwrap_or_else(|| default.into()),
        )
    };
    if mode == Mode::Local {
        return Ok(Profile {
            mode,
            base_url: String::new(),
            model: read("local_model", "gigaam-v3-e2e-ctc")?,
            proxy: String::new(),
            language: read("local_language", "ru")?,
        });
    }
    Ok(Profile {
        mode,
        base_url: read(
            "base_url",
            if mode == Mode::Server {
                "http://localhost:8000/v1"
            } else {
                "https://api.aitunnel.ru/v1"
            },
        )?,
        model: read(
            "model",
            if mode == Mode::Server {
                "whisper-1"
            } else {
                "whisper-large-v3-turbo"
            },
        )?,
        proxy: read("proxy", "")?,
        language: read("language", "ru")?,
    })
}

pub fn validate_profile(p: &Profile) -> Result<(), String> {
    if p.mode == Mode::Local {
        if p.language == "en" {
            if p.model != crate::stt::moonshine::catalog::ID {
                return Err("Select an English recognition model.".into());
            }
        } else if p.language == "ru" {
            crate::stt::local::catalog::model(&p.model)?;
        } else {
            return Err("Select English or Russian for local recognition.".into());
        }
        return Ok(());
    }
    if p.model.trim().is_empty() {
        return Err("Укажите модель распознавания.".into());
    }
    if !matches!(p.language.as_str(), "ru" | "en" | "auto") {
        return Err("Недопустимый язык распознавания.".into());
    }
    validate_remote_connection(p)
}

fn validate_remote_connection(p: &Profile) -> Result<(), String> {
    if p.mode == Mode::Server {
        validate_server_url(&p.base_url)?;
    } else {
        validation::validate_base_url(&p.base_url)?;
    }
    if !p.proxy.trim().is_empty() {
        validation::validate_proxy(&p.proxy)?;
    }
    let url = reqwest::Url::parse(&p.base_url).map_err(|_| "Некорректный адрес сервера.")?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Адрес сервера должен быть без логина, пароля, параметров и фрагмента.".into());
    }
    Ok(())
}

/// HTTP is permitted for explicit RFC1918/ULA/loopback IPs. DNS names require
/// HTTPS except localhost, so public DNS rebinding cannot expand this allowance.
pub fn validate_server_url(raw: &str) -> Result<(), String> {
    if validation::validate_base_url(raw).is_ok() {
        return Ok(());
    }
    let url = reqwest::Url::parse(raw).map_err(|_| "Некорректный адрес сервера.".to_string())?;
    let private = match url
        .host_str()
        .unwrap_or("")
        .trim_matches(['[', ']'])
        .parse::<std::net::IpAddr>()
    {
        Ok(std::net::IpAddr::V4(ip)) => ip.is_private() || ip.is_loopback(),
        Ok(std::net::IpAddr::V6(ip)) => ip.is_unique_local() || ip.is_loopback(),
        _ => false,
    };
    if url.scheme() == "http" && private {
        Ok(())
    } else {
        Err(
            "Используйте HTTPS. HTTP разрешён для localhost и частного IP-адреса офисного сервера."
                .into(),
        )
    }
}

fn normalized_endpoint(raw: &str) -> &str {
    raw.trim().trim_end_matches('/')
}

pub fn profile_key(conn: &Connection, p: &Profile) -> Result<Option<String>, String> {
    if p.mode == Mode::Local {
        return Ok(None);
    }
    let bound = repository::get_setting(conn, &format!("{}_key_endpoint", p.mode.prefix()))
        .map_err(|e| e.to_string())?;
    if bound
        .as_ref()
        .is_some_and(|b| normalized_endpoint(b) != normalized_endpoint(&p.base_url))
    {
        return Ok(None);
    }
    if let Some((endpoint, key)) =
        keyring::get_bound_stt_key(p.mode == Mode::Server).map_err(|e| e.to_string())?
    {
        return Ok(
            (normalized_endpoint(&endpoint) == normalized_endpoint(&p.base_url)).then_some(key),
        );
    }
    (if p.mode == Mode::Server {
        keyring::get_server_stt_key()
    } else {
        keyring::get_stt_key()
    })
    .map_err(|e| e.to_string())
}

pub fn remote_provider(conn: &Connection, p: &Profile) -> Result<OpenAiCompatStt, String> {
    validate_profile(p)?;
    let key = profile_key(conn, p)?;
    if p.mode == Mode::Cloud
        && crate::dictation::session::requires_missing_key(&p.base_url, key.is_some())
    {
        return Err("Добавьте ключ облачного сервиса в настройках диктовки.".into());
    }
    Ok(build_remote_provider(p, key)?.with_prompt(Some(
        if p.language == "en" {
            "Glagol."
        } else {
            crate::stt::DICTATION_PROMPT
        }
        .into(),
    )))
}

fn build_remote_provider(p: &Profile, key: Option<String>) -> Result<OpenAiCompatStt, String> {
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(20));
    // Office traffic must not accidentally travel through an OS-wide cloud proxy.
    if p.mode == Mode::Server {
        builder = builder.no_proxy();
    }
    if !p.proxy.trim().is_empty() {
        builder = builder.proxy(
            reqwest::Proxy::all(validation::normalize_proxy(&p.proxy)?)
                .map_err(|_| "Некорректный адрес прокси.")?,
        );
    }
    let client = builder
        .build()
        .map_err(|_| "Не удалось создать HTTP-клиент.")?;
    Ok(OpenAiCompatStt::new(client, &p.base_url, &p.model, key))
}

fn normalize_profile(profile: &mut Profile) {
    profile.base_url = normalized_endpoint(&profile.base_url).into();
    profile.model = profile.model.trim().into();
    profile.proxy = profile.proxy.trim().into();
}

fn server_draft_provider(
    db: &std::sync::Mutex<Connection>,
    profile: &Profile,
    new_key: Option<&str>,
) -> Result<OpenAiCompatStt, String> {
    let key = if let Some(new_key) = new_key {
        Some(new_key.into())
    } else {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let saved = read_profile(&conn, Some(Mode::Server))?;
        // Even an old, unbound key may only serve its current saved endpoint.
        if normalized_endpoint(&saved.base_url) == profile.base_url {
            profile_key(&conn, &saved)?
        } else {
            None
        }
    };
    build_remote_provider(profile, key)
}

fn model_probe_message(error: ModelProbeError) -> String {
    use crate::preferences::message;
    match error {
        ModelProbeError::Unreachable => message(
            "The server did not respond. Check its address and the connection.",
            "Сервер не отвечает. Проверьте адрес и подключение.",
        ),
        ModelProbeError::KeyRejected => message(
            "The server did not accept the API key. Check the key.",
            "Сервер не принял ключ API. Проверьте ключ.",
        ),
        ModelProbeError::ModelNotFound => message(
            "The selected model was not found on the server.",
            "Выбранная модель на сервере не найдена.",
        ),
        ModelProbeError::RecognitionFailed => message(
            "Test recognition failed. Check the model and language and try again.",
            "Пробное распознавание не удалось. Проверьте модель и язык и попробуйте снова.",
        ),
    }
}

async fn check_server_model(
    provider: &OpenAiCompatStt,
    profile: &Profile,
) -> Result<(), ModelProbeError> {
    let language = (profile.language != "auto").then_some(profile.language.as_str());
    provider
        .probe_transcription(super::dictation::probe_clip(), language)
        .await?;

    // Some servers, including Glagol, transcribe with their current model
    // regardless of the supplied name. Confirm the selected ID when there
    // is a usable catalog. Listing remains optional after a successful probe.
    if provider.list_models().await.is_ok_and(|ids| {
        ids.iter().any(|id| !id.trim().is_empty()) && !ids.contains(&profile.model)
    }) {
        return Err(ModelProbeError::ModelNotFound);
    }
    Ok(())
}

/// Load model IDs using a draft endpoint and credential without saving either.
#[tauri::command]
pub async fn list_server_speech_models(
    state: tauri::State<'_, AppState>,
    mut profile: Profile,
    api_key: Option<String>,
) -> Result<Vec<String>, String> {
    (async {
        if profile.mode != Mode::Server {
            return Err("Organization server mode is required.".into());
        }
        normalize_profile(&mut profile);
        validate_remote_connection(&profile)?;
        let new_key = api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty());
        let provider = server_draft_provider(&state.db, &profile, new_key)?;
        match provider.list_models().await {
            Ok(mut ids) => {
                ids.retain(|id| !id.trim().is_empty());
                ids.sort_unstable();
                ids.dedup();
                Ok(ids)
            }
            Err(SttError::Api(message))
                if ["HTTP 404:", "HTTP 405:", "HTTP 501:"]
                    .iter()
                    .any(|prefix| message.starts_with(prefix)) =>
            {
                Ok(Vec::new())
            }
            Err(SttError::Auth) => Err(model_probe_message(ModelProbeError::KeyRejected)),
            Err(SttError::Network(err)) => Err(model_probe_message(if err.is_builder() {
                ModelProbeError::KeyRejected
            } else {
                ModelProbeError::Unreachable
            })),
            Err(_) => Err(crate::preferences::message(
                "Could not load the model list. You can enter the model name manually.",
                "Не удалось загрузить список моделей. Можно ввести имя модели вручную.",
            )),
        }
    })
    .await
    .map_err(crate::i18n::error)
}

#[tauri::command]
pub fn get_speech_settings(
    state: tauri::State<'_, AppState>,
    mode: Option<Mode>,
) -> Result<Settings, String> {
    (|| {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let profile = read_profile(&conn, mode)?;
        let key_stored = profile_key(&conn, &profile)?.is_some();
        Ok(Settings {
            model_status: speech_model_check::saved_status(&conn, profile.mode)?,
            profile,
            key_stored,
            active_mode: active_mode(&conn)?,
        })
    })()
    .map_err(crate::i18n::error)
}

/// Store a profile atomically, preserving the other remote profile and weights.
pub fn persist_profile(conn: &mut Connection, p: &Profile, new_key: bool) -> Result<(), String> {
    validate_profile(p)?;
    let now = chrono::Utc::now().timestamp_millis();
    let previous = read_profile(conn, Some(p.mode))?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let set = |key: &str, value: &str| {
        repository::set_setting(&tx, key, value, now).map_err(|e| e.to_string())
    };
    if p.mode == Mode::Local {
        set("stt_local_model", &p.model)?;
        set("stt_local_language", &p.language)?;
        set(&format!("stt_local_model_{}", p.language), &p.model)?;
    } else {
        let binding = format!("{}_key_endpoint", p.mode.prefix());
        if new_key {
            set(&binding, &p.base_url)?;
        } else if repository::get_setting(&tx, &binding)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            set(&binding, normalized_endpoint(&previous.base_url))?;
        }
        for (key, value) in [
            ("base_url", p.base_url.as_str()),
            ("model", p.model.as_str()),
            ("proxy", p.proxy.as_str()),
            ("language", p.language.as_str()),
        ] {
            set(&format!("{}_{}", p.mode.prefix(), key), value)?;
        }
    }
    if p.mode != Mode::Local && (new_key || previous != *p) {
        speech_model_check::invalidate_warning(&tx, p.mode)?;
    }
    set("stt_mode", p.mode.name())?;
    set(
        "stt_provider",
        if p.mode == Mode::Local {
            "На этом компьютере"
        } else if p.mode == Mode::Server {
            "Сервер организации"
        } else {
            "Облачный сервис"
        },
    )?;
    tx.commit().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_speech_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mut profile: Profile,
    api_key: Option<String>,
) -> Result<(), String> {
    (async {
        normalize_profile(&mut profile);
        validate_profile(&profile)?;
        if *state.dictation.lock().map_err(|e| e.to_string())?
            != crate::dictation::DictationPhase::Idle
        {
            return Err("Wait for the current dictation to finish.".into());
        }
        let models = app.state::<Arc<LocalModels>>().inner().clone();
        let _operation = models
            .operation
            .try_lock()
            .map_err(|_| "Дождитесь операции с локальной моделью.".to_string())?;
        if profile.mode == Mode::Local && profile.language == "ru" {
            let models = models.clone();
            let id = profile.model.clone();
            tauri::async_runtime::spawn_blocking(move || models.prepare(&id))
                .await
                .map_err(|e| e.to_string())??;
        }
        let new_key = api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty());
        if profile.mode == Mode::Server {
            let provider = server_draft_provider(&state.db, &profile, new_key)?;
            check_server_model(&provider, &profile)
                .await
                .map_err(model_probe_message)?;
        }
        {
            let phase = state.dictation.lock().map_err(|e| e.to_string())?;
            if *phase != crate::dictation::DictationPhase::Idle {
                return Err("Wait for the current dictation to finish.".into());
            }
            let mut conn = state.db.lock().map_err(|e| e.to_string())?;
            let key = (profile.mode != Mode::Local).then_some(new_key).flatten();
            let is_server = profile.mode == Mode::Server;
            let mut previous_key = None;
            if let Some(key) = key {
                previous_key = keyring::get_bound_stt_key(is_server).map_err(|e| e.to_string())?;
                keyring::set_bound_stt_key(is_server, &profile.base_url, key)
                    .map_err(|e| e.to_string())?;
            }
            if let Err(error) = persist_profile(&mut conn, &profile, key.is_some()) {
                if key.is_some() {
                    let restored = match previous_key {
                        Some((endpoint, previous_key)) => {
                            keyring::set_bound_stt_key(is_server, &endpoint, &previous_key)
                        }
                        None => keyring::delete_bound_stt_key(is_server),
                    };
                    restored.map_err(|e| e.to_string())?;
                }
                return Err(error);
            }
            if let Ok(preferences) = crate::preferences::get(&conn) {
                let _ = app.emit("preferences-changed", &preferences);
            }
        }
        *state.stt_key_validated.lock().await = false;
        if profile.mode != Mode::Local {
            let models = models.clone();
            tauri::async_runtime::spawn_blocking(move || {
                *models.engine.lock().map_err(|e| e.to_string())? = None;
                Ok::<_, String>(())
            })
            .await
            .map_err(|e| e.to_string())??;
        }
        let _ = app.emit("speech-settings-changed", ());
        Ok(())
    })
    .await
    .map_err(crate::i18n::error)
}

#[tauri::command]
pub async fn test_speech_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    (async {
        let (p, remote) = {
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            let p = read_profile(&conn, None)?;
            let remote = if p.mode != Mode::Local {
                Some(remote_provider(&conn, &p)?)
            } else {
                None
            };
            (p, remote)
        };
        if let Some(provider) = remote {
            super::dictation::check_provider(&provider, &p.language).await
        } else {
            let models = app.state::<Arc<LocalModels>>().inner().clone();
            if p.language == "en" {
                let stream = models.moonshine.begin().await?;
                stream.input.finish()?;
                let result = stream.result.lock().unwrap().take().unwrap();
                result.await.map_err(|e| e.to_string())??;
                return Ok(());
            }
            tauri::async_runtime::spawn_blocking(move || models.prepare(&p.model))
                .await
                .map_err(|e| e.to_string())?
        }
    })
    .await
    .map_err(crate::i18n::error)
}

#[tauri::command]
pub fn delete_speech_key(state: tauri::State<'_, AppState>, mode: Mode) -> Result<(), String> {
    (|| {
        if mode == Mode::Local {
            return Ok(());
        }
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        match keyring::delete_bound_stt_key(mode == Mode::Server) {
            Ok(()) | Err(keyring::KeyringError::NotFound) => {}
            Err(e) => return Err(e.to_string()),
        }
        let result = if mode == Mode::Server {
            keyring::delete_server_stt_key()
        } else {
            keyring::delete_stt_key()
        };
        match result {
            Ok(()) | Err(keyring::KeyringError::NotFound) => {
                speech_model_check::invalidate_warning(&conn, mode)
            }
            Err(e) => Err(e.to_string()),
        }
    })()
    .map_err(crate::i18n::error)
}

pub fn build_backend(
    app: &tauri::AppHandle,
    conn: &Connection,
) -> Result<(SttBackend, Profile), String> {
    let p = read_profile(conn, None)?;
    let backend = if p.mode == Mode::Local {
        if p.language == "en" {
            validate_profile(&p)?;
            return Ok((
                SttBackend::Moonshine(crate::stt::moonshine::Provider::new(
                    app.state::<Arc<LocalModels>>().moonshine.clone(),
                )),
                p,
            ));
        }
        crate::stt::local::catalog::model(&p.model)?;
        SttBackend::Local(LocalProvider {
            models: app.state::<Arc<LocalModels>>().inner().clone(),
            id: p.model.clone(),
        })
    } else {
        SttBackend::OpenAiCompat(remote_provider(conn, &p)?)
    };
    Ok((backend, p))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn draft_profile(server: &mockito::Server) -> Profile {
        Profile {
            mode: Mode::Server,
            base_url: format!("{}/v1", server.url()),
            model: "selected-model".into(),
            proxy: String::new(),
            language: "ru".into(),
        }
    }

    #[tokio::test]
    async fn manual_server_model_works_without_a_model_list() {
        let mut server = mockito::Server::new_async().await;
        let profile = draft_profile(&server);
        let probe = server
            .mock("POST", "/v1/audio/transcriptions")
            .match_header("authorization", "Bearer draft-key")
            .match_body(mockito::Matcher::AllOf(vec![
                mockito::Matcher::Regex("name=\"file\"; filename=\"audio.wav\"".into()),
                mockito::Matcher::Regex("selected-model".into()),
            ]))
            .with_body(r#"{"text":""}"#)
            .create_async()
            .await;
        let models = server
            .mock("GET", "/v1/models")
            .with_status(404)
            .create_async()
            .await;
        let db = std::sync::Mutex::new(crate::db::test_connection());
        let provider = server_draft_provider(&db, &profile, Some("draft-key")).unwrap();
        assert_eq!(check_server_model(&provider, &profile).await, Ok(()));
        probe.assert_async().await;
        models.assert_async().await;
    }

    #[tokio::test]
    async fn rejects_auth_missing_model_and_failed_recognition() {
        for (status, body, expected) in [
            (403, "forbidden", ModelProbeError::KeyRejected),
            (
                404,
                r#"{"error":{"code":"model_not_found"}}"#,
                ModelProbeError::ModelNotFound,
            ),
            (429, "busy", ModelProbeError::RecognitionFailed),
            (500, "unavailable", ModelProbeError::RecognitionFailed),
            (
                200,
                "not a transcription",
                ModelProbeError::RecognitionFailed,
            ),
        ] {
            let mut server = mockito::Server::new_async().await;
            let profile = draft_profile(&server);
            let probe = server
                .mock("POST", "/v1/audio/transcriptions")
                .with_status(status)
                .with_body(body)
                .create_async()
                .await;
            let provider = build_remote_provider(&profile, None).unwrap();
            assert_eq!(check_server_model(&provider, &profile).await, Err(expected));
            probe.assert_async().await;
        }
    }

    #[tokio::test]
    async fn a_successful_probe_cannot_activate_an_unadvertised_model() {
        let mut server = mockito::Server::new_async().await;
        let profile = draft_profile(&server);
        let _probe = server
            .mock("POST", "/v1/audio/transcriptions")
            .with_body(r#"{"text":""}"#)
            .create_async()
            .await;
        let _models = server
            .mock("GET", "/v1/models")
            .with_body(r#"{"object":"list","data":[{"id":"current-model","owned_by":"glagol"}]}"#)
            .create_async()
            .await;
        let provider = build_remote_provider(&profile, None).unwrap();
        assert_eq!(
            check_server_model(&provider, &profile).await,
            Err(ModelProbeError::ModelNotFound)
        );
    }

    #[test]
    fn profiles_preserve_cloud_when_selecting_server_and_local() {
        let mut conn = crate::db::test_connection();
        let cloud = read_profile(&conn, None).unwrap();
        let mut server = read_profile(&conn, Some(Mode::Server)).unwrap();
        server.base_url = "http://192.168.1.42:8000/v1".into();
        persist_profile(&mut conn, &server, false).unwrap();
        assert_eq!(read_profile(&conn, None).unwrap().base_url, server.base_url);
        let local = read_profile(&conn, Some(Mode::Local)).unwrap();
        persist_profile(&mut conn, &local, false).unwrap();
        assert_eq!(
            read_profile(&conn, Some(Mode::Cloud)).unwrap().base_url,
            cloud.base_url
        );
        assert_eq!(
            read_profile(&conn, Some(Mode::Server)).unwrap().base_url,
            server.base_url
        );
    }
    #[test]
    fn office_http_accepts_only_explicit_private_addresses() {
        for url in [
            "http://192.168.1.2/v1",
            "http://10.0.0.2/v1",
            "http://172.16.0.1/v1",
            "http://[fd12::1]/v1",
            "http://localhost:8000/v1",
            "https://office.example/v1",
        ] {
            assert!(validate_server_url(url).is_ok(), "{url}");
        }
        for url in [
            "http://8.8.8.8/v1",
            "http://office.example/v1",
            "http://169.254.169.254",
            "http://172.32.0.1",
            "ftp://192.168.1.1",
        ] {
            assert!(validate_server_url(url).is_err(), "{url}");
        }
    }
    #[test]
    fn changing_endpoint_does_not_rebind_existing_secret() {
        let mut conn = crate::db::test_connection();
        let mut p = read_profile(&conn, None).unwrap();
        let old = p.base_url.clone();
        p.base_url = "https://new.example/v1".into();
        persist_profile(&mut conn, &p, false).unwrap();
        assert_eq!(
            repository::get_setting(&conn, "stt_key_endpoint").unwrap(),
            Some(old)
        );
        assert_eq!(profile_key(&conn, &p).unwrap(), None);
        persist_profile(&mut conn, &p, true).unwrap();
        assert_eq!(
            repository::get_setting(&conn, "stt_key_endpoint").unwrap(),
            Some(p.base_url)
        );
    }
}
