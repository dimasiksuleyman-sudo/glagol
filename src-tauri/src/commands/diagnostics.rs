//! Settings → Diagnostics: the «Detailed log» switch and the log folder.
use crate::state::AppState;
use serde::Serialize;
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

/// What the Diagnostics section shows.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSettings {
    /// «Detailed log» is on.
    pub detailed: bool,
    /// Folder with the daily log files (shown so it can be found manually).
    pub log_dir: String,
}

fn settings(app: &tauri::AppHandle, detailed: bool) -> Result<DiagnosticsSettings, String> {
    let dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    Ok(DiagnosticsSettings {
        detailed,
        log_dir: dir.to_string_lossy().into_owned(),
    })
}

/// Read the switch state and the log folder.
#[tauri::command]
pub fn get_diagnostics_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<DiagnosticsSettings, String> {
    (|| {
        let detailed = {
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            crate::db::repository::get_setting(&conn, crate::diagnostics::DETAILED_SETTING)
                .map_err(|e| e.to_string())?
                .as_deref()
                == Some("1")
        };
        settings(&app, detailed)
    })()
    .map_err(crate::i18n::error)
}

/// Persist and apply «Detailed log» without restarting.
#[tauri::command]
pub fn set_detailed_logging(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<DiagnosticsSettings, String> {
    (|| {
        {
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            crate::diagnostics::save(&conn, enabled).map_err(|e| e.to_string())?;
        }
        if enabled {
            crate::diagnostics::log_system();
        }
        settings(&app, enabled)
    })()
    .map_err(crate::i18n::error)
}

/// Open the log folder in the system file manager.
#[tauri::command]
pub fn open_log_folder(app: tauri::AppHandle) -> Result<(), String> {
    (|| {
        let dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        app.opener()
            .open_path(dir.to_string_lossy(), None::<&str>)
            .map_err(|e| e.to_string())
    })()
    .map_err(crate::i18n::error)
}
