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

fn normalize_base_url(raw: &str) -> Result<String, String> {
    ai::endpoint(raw.trim())
        .filter(|url| url.as_str().len() <= 256)
        .map(|url| url.as_str().trim_end_matches('/').to_string())
        .ok_or_else(|| {
            message(
                "Enter a valid HTTPS address, for example https://routerai.ru/api/v1.",
                "Укажите корректный адрес HTTPS, например https://routerai.ru/api/v1.",
            )
        })
}

fn clean_key(value: &str) -> Result<Option<String>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > 512 || !value.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(message(
            "Enter a valid API key.",
            "Укажите корректный ключ API.",
        ));
    }
    Ok(Some(value.into()))
}

/// A new key, or the saved one when it belongs to the same address.
fn key_for(base_url: &str, api_key: Option<String>) -> Result<Option<String>, String> {
    match api_key {
        Some(value) => clean_key(&value),
        None => Ok(configuration()?
            .filter(|saved| saved.settings.base_url == base_url)
            .and_then(|saved| saved.api_key)),
    }
}

fn models_error(error: ai::ModelsError) -> String {
    match error {
        ai::ModelsError::BadAddress => normalize_base_url("").unwrap_err(),
        ai::ModelsError::Unreachable => message(
            "The address does not answer. Check it and your internet connection.",
            "Адрес не отвечает. Проверьте его и подключение к интернету.",
        ),
        ai::ModelsError::KeyRejected => message(
            "The provider did not accept the key.",
            "Провайдер не принял ключ.",
        ),
        ai::ModelsError::NoModelList => message(
            "This address does not provide an OpenAI-compatible model list. Check the address (it usually ends with /v1) or enter the model name manually.",
            "По этому адресу нет OpenAI-совместимого списка моделей. Проверьте адрес (обычно он заканчивается на /v1) или впишите название модели вручную.",
        ),
    }
}

/// Models offered by an endpoint and the one Glagol suggests.
#[derive(Serialize)]
pub struct ModelsView {
    models: Vec<String>,
    suggested: Option<String>,
}

/// Read the endpoint's model list with the given (or saved) key.
#[tauri::command]
pub async fn list_ai_models(
    state: State<'_, AppState>,
    base_url: String,
    api_key: Option<String>,
) -> Result<ModelsView, String> {
    let base_url = normalize_base_url(&base_url)?;
    let key = key_for(&base_url, api_key)?
        .ok_or_else(|| message("Enter the API key.", "Укажите ключ API."))?;
    let models = state
        .ai_normalizer
        .list_models(&base_url, &key)
        .await
        .map_err(models_error)?;
    Ok(ModelsView {
        suggested: ai::suggest_model(&models),
        models,
    })
}

/// Full check of a configuration: key, model availability (when the
/// endpoint lists models) and a built-in sample phrase.
async fn check(state: &AppState, config: &Configuration) -> Result<(), String> {
    let key = config
        .api_key
        .as_deref()
        .ok_or_else(|| message("Enter the API key.", "Укажите ключ API."))?;
    match state
        .ai_normalizer
        .list_models(&config.settings.base_url, key)
        .await
    {
        Ok(models) if !models.contains(&config.settings.model) => {
            return Err(message(
                "The provider has no such model now. Choose another model from the list.",
                "У провайдера сейчас нет такой модели. Выберите другую из списка.",
            ))
        }
        Ok(_) | Err(ai::ModelsError::NoModelList) => {}
        Err(error) => return Err(models_error(error)),
    }
    if state.ai_normalizer.test_connection(config).await {
        Ok(())
    } else {
        Err(message(
            "The provider answered but did not normalize the test phrase. Try another model.",
            "Провайдер ответил, но не обработал проверочную фразу. Попробуйте другую модель.",
        ))
    }
}

/// Save the settings. When AI normalization is on, the address, key and
/// model are checked first and nothing is saved if the check fails.
#[tauri::command]
pub async fn save_ai_normalization_settings(
    state: State<'_, AppState>,
    mut settings: Settings,
    api_key: Option<String>,
) -> Result<SettingsView, String> {
    settings.base_url = normalize_base_url(&settings.base_url)?;
    settings.model = settings.model.trim().into();
    if settings.model.is_empty()
        || settings.model.len() > 128
        || settings.model.chars().any(char::is_control)
    {
        return Err(message(
            "Choose a model (up to 128 bytes).",
            "Выберите модель (до 128 байт).",
        ));
    }
    let key = key_for(&settings.base_url, api_key)?;
    if settings.enabled && key.is_none() {
        return Err(message(
            "Enter an API key for this address.",
            "Укажите ключ API для этого адреса.",
        ));
    }
    let config = Configuration {
        settings,
        api_key: key,
    };
    if config.settings.enabled {
        check(&state, &config).await?;
    }
    let value = serde_json::to_string(&config).map_err(|_| credential_error())?;
    keyring::set_ai_normalization(&value).map_err(|_| credential_error())?;
    state.ai_normalizer.clear_cache();
    Ok(SettingsView {
        settings: config.settings,
        has_key: config.api_key.is_some(),
    })
}

/// Check the saved configuration again (models change and get retired).
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
    check(&state, &config).await
}

pub(crate) async fn prepare_document(
    state: &AppState,
    config: Option<&Configuration>,
    text: &str,
    language: &str,
) -> Result<Prepared, String> {
    let (entries, strip_brackets) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        (
            super::pronunciations::for_synthesis(&conn),
            crate::preferences::get(&conn)
                .map(|p| p.tts_strip_brackets)
                .unwrap_or(false),
        )
    };
    // «Do not read text in brackets» comes first: asides never reach the
    // dictionary, the optional AI step or the engine.
    let text = if strip_brackets {
        crate::text::brackets::strip(text)
    } else {
        text.to_string()
    };
    Ok(ai::prepare(&state.ai_normalizer, config, &text, &entries, language).await)
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
