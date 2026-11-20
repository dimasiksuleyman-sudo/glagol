//! Settings, endpoint test and document preview for optional AI normalization.
use crate::{
    preferences::{message, tts_enabled},
    secrets::keyring,
    state::AppState,
    text::ai_normalization::{self as ai, Configuration, Prepared, Settings},
};
use serde::Serialize;
use tauri::State;

pub(crate) const MAX_TEXT_CHARS: usize = 500_000;

#[derive(Serialize)]
/// IPC response with the public configuration and key availability.
pub struct SettingsView {
    settings: Settings,
    has_key: bool,
}

fn credential_error() -> String {
    message(
        "Could not read or save the AI settings in your system credential store.",
        "Не удалось прочитать или сохранить настройки ИИ в системном хранилище.",
    )
}

pub(crate) fn configuration() -> Result<Option<Configuration>, String> {
    keyring::get_ai_normalization()
        .map_err(|_| credential_error())?
        .map(|stored| serde_json::from_str(&stored).map_err(|_| credential_error()))
        .transpose()
}

/// Read AI settings without exposing the API key.
#[tauri::command]
pub fn get_ai_normalization_settings() -> Result<SettingsView, String> {
    Ok(match configuration()? {
        Some(config) => SettingsView {
            settings: config.settings,
            has_key: config.api_key.is_some(),
        },
        None => SettingsView {
            settings: Settings::default(),
            has_key: false,
        },
    })
}

/// Save consent and settings with a key bound to the selected URL.
#[tauri::command]
pub fn save_ai_normalization_settings(
    state: State<'_, AppState>,
    mut settings: Settings,
    api_key: Option<String>,
) -> Result<SettingsView, String> {
    if settings.provider != ai::Provider::Custom {
        settings.base_url = settings.provider.endpoint().into();
    }
    settings.model = settings.model.trim().into();
    settings.base_url = settings.base_url.trim().into();
    let url = ai::endpoint(&settings.base_url).filter(|url| url.as_str().len() <= 256);
    settings.base_url = url
        .ok_or_else(|| {
            message(
                "Enter a valid HTTPS endpoint.",
                "Укажите корректный адрес HTTPS.",
            )
        })?
        .as_str()
        .trim_end_matches('/')
        .into();
    if settings.model.is_empty()
        || settings.model.len() > 128
        || settings.model.chars().any(char::is_control)
    {
        return Err(message(
            "Enter a model name (up to 128 bytes).",
            "Укажите название модели (до 128 байт).",
        ));
    }
    let key = match api_key {
        Some(value) if value.trim().is_empty() => None,
        Some(value) => {
            let value = value.trim();
            if value.len() > 512 || !value.bytes().all(|b| b.is_ascii_graphic()) {
                return Err(message(
                    "Enter a valid API key.",
                    "Укажите корректный ключ API.",
                ));
            }
            Some(value.into())
        }
        None => configuration()?
            .filter(|saved| saved.settings.base_url == settings.base_url)
            .and_then(|saved| saved.api_key),
    };
    if settings.enabled && key.is_none() {
        return Err(message(
            "Enter an API key for this endpoint.",
            "Укажите ключ API для этого адреса.",
        ));
    }
    let config = Configuration {
        settings,
        api_key: key,
    };
    let value = serde_json::to_string(&config).map_err(|_| credential_error())?;
    keyring::set_ai_normalization(&value).map_err(|_| credential_error())?;
    state.ai_normalizer.clear_cache();
    Ok(SettingsView {
        settings: config.settings,
        has_key: config.api_key.is_some(),
    })
}

/// Send a built-in sample to the saved provider.
#[tauri::command]
pub async fn test_ai_normalization(state: State<'_, AppState>) -> Result<(), String> {
    let config = configuration()?
        .filter(|config| config.api_key.is_some())
        .ok_or_else(|| {
            message(
                "Save an API key before testing.",
                "Сохраните ключ API перед проверкой.",
            )
        })?;
    if state.ai_normalizer.test_connection(&config).await {
        Ok(())
    } else {
        Err(message(
            "The provider did not normalize the test phrase. Check the endpoint, key and model.",
            "Провайдер не обработал проверочную фразу. Проверьте адрес, ключ и модель.",
        ))
    }
}

pub(crate) async fn prepare_document(
    state: &AppState,
    config: Option<&Configuration>,
    text: &str,
    language: &str,
) -> Result<Prepared, String> {
    let entries = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        super::pronunciations::for_synthesis(&conn)
    };
    Ok(ai::prepare(&state.ai_normalizer, config, text, &entries, language).await)
}

/// Run the same preparation pipeline as document synthesis.
#[tauri::command]
pub async fn preview_document_normalization(
    state: State<'_, AppState>,
    text: String,
    language: String,
) -> Result<Prepared, String> {
    if !tts_enabled() {
        return Err(message(
            "Text to speech is turned off.",
            "Озвучка выключена.",
        ));
    }
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(crate::i18n::error(
            "Текст слишком большой: максимум 500 000 символов.".into(),
        ));
    }
    let config = configuration().ok().flatten();
    prepare_document(
        &state,
        config.as_ref(),
        &text,
        if language == "en" { "en" } else { "ru" },
    )
    .await
    .map_err(crate::i18n::error)
}
