//! Persisted UI language and onboarding, shared by every window.
use crate::{
    preferences::{self, Language, Preferences},
    state::AppState,
};
use tauri::Emitter;

#[tauri::command]
pub fn is_dictating(state: tauri::State<'_, AppState>) -> bool {
    state
        .dictation
        .lock()
        .is_ok_and(|phase| *phase != crate::dictation::DictationPhase::Idle)
}

#[tauri::command]
pub fn set_stt_preference(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    models: tauri::State<'_, std::sync::Arc<crate::stt::local::LocalModels>>,
    language: Language,
) -> Result<Preferences, String> {
    (|| {
        let phase = state.dictation.lock().map_err(|e| e.to_string())?;
        if *phase != crate::dictation::DictationPhase::Idle {
            return Err("Wait for the current dictation to finish.".into());
        }
        let _operation = models
            .operation
            .try_lock()
            .map_err(|_| "Wait for the current model operation to finish.")?;
        let value = {
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            preferences::set_stt_language(&conn, language).map_err(|e| e.to_string())?;
            preferences::get(&conn).map_err(|e| e.to_string())?
        };
        let _ = app.emit("preferences-changed", &value);
        let _ = app.emit("speech-settings-changed", ());
        Ok(value)
    })()
    .map_err(crate::i18n::error)
}

#[tauri::command]
pub fn set_tts_preference(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    models: tauri::State<'_, crate::tts::silero::models::Models>,
    language: Language,
    voice: Option<String>,
) -> Result<Preferences, String> {
    (|| {
        let _operation = models.ru.operation.try_lock().map_err(|_| {
            preferences::message(
                "Wait for the current speech operation to finish.",
                "Дождитесь завершения текущей операции озвучки.",
            )
        })?;
        let value = {
            let conn = state.db.lock().unwrap();
            if let Some(voice) = voice {
                preferences::set_tts_voice(&conn, language, &voice)?;
            } else {
                preferences::set_tts_language(&conn, language).map_err(|e| e.to_string())?;
            }
            preferences::get(&conn).map_err(|e| e.to_string())?
        };
        let _ = app.emit("preferences-changed", &value);
        let _ = app.emit("tts-status", ());
        Ok(value)
    })()
    .map_err(crate::i18n::error)
}

#[tauri::command]
pub fn get_preferences(state: tauri::State<'_, AppState>) -> Result<Preferences, String> {
    preferences::get(&state.db.lock().unwrap()).map_err(|e| crate::i18n::error(e.to_string()))
}
#[tauri::command]
pub fn set_ui_language(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    language: Language,
) -> Result<Preferences, String> {
    (|| {
        let value = preferences::choose_ui(&state.db.lock().unwrap(), language)
            .map_err(|e| e.to_string())?;
        preferences::activate(language);
        crate::dictation::session::refresh_tray_language(&app).map_err(|e| e.to_string())?;
        let _ = app.emit("preferences-changed", &value);
        Ok(value)
    })()
    .map_err(crate::i18n::error)
}
#[tauri::command]
pub fn migrate_legacy_voice(
    state: tauri::State<'_, AppState>,
    voice: Option<String>,
) -> Result<Preferences, String> {
    (|| {
        let conn = state.db.lock().unwrap();
        preferences::migrate_voice(&conn, voice.as_deref()).map_err(|e| e.to_string())?;
        preferences::get(&conn).map_err(|e| e.to_string())
    })()
    .map_err(crate::i18n::error)
}
#[tauri::command]
pub fn complete_onboarding(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Preferences, String> {
    (|| {
        let value = {
            let conn = state.db.lock().unwrap();
            preferences::finish_onboarding(&conn).map_err(|e| e.to_string())?;
            preferences::get(&conn).map_err(|e| e.to_string())?
        };
        let _ = app.emit("preferences-changed", &value);
        Ok(value)
    })()
    .map_err(crate::i18n::error)
}
