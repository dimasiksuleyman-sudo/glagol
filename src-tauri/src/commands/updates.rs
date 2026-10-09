//! Application version and in-app updates (0.6.0).
//!
//! The updater plugin is used from Rust only; the webview gets no plugin
//! permissions. `check_for_update` fetches `latest.json` from the GitHub
//! Release and keeps the found update in [`PendingUpdate`]; `install_update`
//! downloads it with progress, verifies the minisign signature against the
//! public key in `tauri.conf.json` and runs the NSIS installer, which exits the
//! app on Windows. Network access happens only on an explicit user action or,
//! when the user enabled it, one automatic check after startup.

use std::sync::Mutex;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::db::repository;
use crate::preferences::message;
use crate::state::AppState;

/// `app_settings` key for the automatic update check. Absent means off.
pub const KEY_AUTO_CHECK: &str = "update_auto_check";
/// Unix ms of the last update reminder shown at startup.
pub const KEY_REMINDER_AT: &str = "update_reminder_at";
/// Unix ms of the last successful update check, manual or automatic.
pub const KEY_LAST_CHECK_AT: &str = "update_last_check_at";

/// Minimum interval between startup reminders about updates.
pub const REMINDER_INTERVAL_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// The update found by the last successful check, waiting for installation.
#[derive(Default)]
pub struct PendingUpdate(pub Mutex<Option<Update>>);

/// Current version and update preferences for the Settings page.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettings {
    pub current_version: String,
    pub auto_check: bool,
    /// Whether the startup reminder (enable auto-check or check now) is due.
    pub reminder_due: bool,
}

/// An available update as shown to the user.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

/// Download progress streamed to the Settings page.
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateProgress {
    Downloading { downloaded: u64, total: Option<u64> },
    Installing,
}

/// Parse the stored auto-check flag; anything but `"true"` is off.
pub fn parse_auto_check(value: Option<&str>) -> bool {
    value == Some("true")
}

/// Whether to remind the user about updates at startup: only while the
/// automatic check is off, and at most once per [`REMINDER_INTERVAL_MS`] since
/// the last reminder or the last successful check. Never shown before → due.
pub fn reminder_due(
    auto_check: bool,
    reminder_at: Option<i64>,
    last_check_at: Option<i64>,
    now: i64,
) -> bool {
    if auto_check {
        return false;
    }
    match reminder_at.max(last_check_at) {
        None => true,
        Some(last) => now.saturating_sub(last) >= REMINDER_INTERVAL_MS,
    }
}

fn read_millis(conn: &rusqlite::Connection, setting_name: &str) -> Result<Option<i64>, String> {
    let value = repository::get_setting(conn, setting_name).map_err(|e| e.to_string())?;
    Ok(value.and_then(|v| v.parse().ok()))
}

fn write_now(conn: &rusqlite::Connection, setting_name: &str) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp_millis();
    repository::set_setting(conn, setting_name, &now.to_string(), now)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Read the version, the auto-check flag and whether the reminder is due.
#[tauri::command]
pub fn get_update_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<UpdateSettings, String> {
    let (auto_check, reminder_due) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let value = repository::get_setting(&conn, KEY_AUTO_CHECK).map_err(|e| e.to_string())?;
        let auto_check = parse_auto_check(value.as_deref());
        let due = reminder_due(
            auto_check,
            read_millis(&conn, KEY_REMINDER_AT)?,
            read_millis(&conn, KEY_LAST_CHECK_AT)?,
            chrono::Utc::now().timestamp_millis(),
        );
        (auto_check, due)
    };
    Ok(UpdateSettings {
        current_version: app.package_info().version.to_string(),
        auto_check,
        reminder_due,
    })
}

/// Record that the startup reminder was shown, so the next one waits a month.
#[tauri::command]
pub fn mark_update_reminder_shown(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    write_now(&conn, KEY_REMINDER_AT)
}

/// Persist the auto-check flag.
#[tauri::command]
pub fn set_update_auto_check(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let value = if enabled { "true" } else { "false" };
    repository::set_setting(
        &conn,
        KEY_AUTO_CHECK,
        value,
        chrono::Utc::now().timestamp_millis(),
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Ask the release endpoint for a newer version. `Ok(None)` means up to date.
#[tauri::command]
pub async fn check_for_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    pending: tauri::State<'_, PendingUpdate>,
) -> Result<Option<UpdateInfo>, String> {
    let updater = app.updater().map_err(|e| {
        tracing::warn!("updater unavailable: {e}");
        check_failed()
    })?;
    let update = updater.check().await.map_err(|e| {
        tracing::warn!("update check failed: {e}");
        check_failed()
    })?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        notes: u.body.clone(),
        date: u.date.map(|d| d.to_string()),
    });
    if let Some(info) = &info {
        tracing::info!(version = %info.version, "update available");
    }
    *pending.0.lock().map_err(|e| e.to_string())? = update;
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        write_now(&conn, KEY_LAST_CHECK_AT)?;
    }
    Ok(info)
}

/// Download, verify and install the pending update. On Windows the installer
/// replaces the app and the current process exits, so a successful call does
/// not return. Refused while dictation, local speech synthesis or a model
/// operation is running, because the installer would terminate it.
#[tauri::command]
pub async fn install_update(
    state: tauri::State<'_, AppState>,
    pending: tauri::State<'_, PendingUpdate>,
    tts: tauri::State<'_, crate::tts::silero::models::Models>,
    stt: tauri::State<'_, std::sync::Arc<crate::stt::local::LocalModels>>,
    on_progress: Channel<UpdateProgress>,
) -> Result<(), String> {
    {
        let phase = state.dictation.lock().map_err(|e| e.to_string())?;
        if *phase != crate::dictation::DictationPhase::Idle {
            return Err(message(
                "Wait for the current dictation to finish.",
                "Дождитесь окончания текущей диктовки.",
            ));
        }
    }
    // Held until the process exits, so no new operation starts mid-install.
    let _tts_operation = tts.ru.operation.try_lock().map_err(|_| busy())?;
    let _stt_operation = stt.operation.try_lock().map_err(|_| busy())?;

    let update = pending
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or_else(|| {
            message(
                "Check for updates first.",
                "Сначала проверьте наличие обновлений.",
            )
        })?;

    let mut downloaded: u64 = 0;
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                let _ = on_progress.send(UpdateProgress::Downloading { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| {
            tracing::warn!("update download or signature check failed: {e}");
            message(
                "The update could not be downloaded or its signature is invalid. Nothing was installed.",
                "Не удалось скачать обновление или его подпись неверна. Ничего не установлено.",
            )
        })?;

    let _ = on_progress.send(UpdateProgress::Installing);
    tracing::info!(version = %update.version, "installing update");
    update.install(bytes).map_err(|e| {
        tracing::warn!("update install failed: {e}");
        message(
            "The update installer could not be started.",
            "Не удалось запустить установщик обновления.",
        )
    })
}

fn check_failed() -> String {
    message(
        "Could not check for updates. Check your internet connection and try again.",
        "Не удалось проверить обновления. Проверьте подключение к интернету и повторите.",
    )
}

fn busy() -> String {
    message(
        "Wait for the current speech operation to finish.",
        "Дождитесь завершения текущей операции озвучки.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_connection;

    #[test]
    fn auto_check_is_off_unless_explicitly_enabled() {
        assert!(!parse_auto_check(None));
        assert!(!parse_auto_check(Some("false")));
        assert!(!parse_auto_check(Some("1")));
        assert!(parse_auto_check(Some("true")));
    }

    #[test]
    fn reminder_is_monthly_and_only_while_auto_check_is_off() {
        let day = 24 * 60 * 60 * 1000;
        let now = 100 * day;
        // First run with the feature: remind once.
        assert!(reminder_due(false, None, None, now));
        // Auto-check on: never remind.
        assert!(!reminder_due(true, None, None, now));
        // Reminded 29 days ago: wait; 30 days ago: due.
        assert!(!reminder_due(false, Some(now - 29 * day), None, now));
        assert!(reminder_due(false, Some(now - 30 * day), None, now));
        // A recent manual check postpones the reminder.
        assert!(!reminder_due(
            false,
            Some(now - 40 * day),
            Some(now - day),
            now
        ));
        assert!(!reminder_due(false, None, Some(now - day), now));
        // Clock moved backwards: no reminder storm.
        assert!(!reminder_due(false, Some(now + day), None, now));
    }

    #[test]
    fn auto_check_round_trips_through_settings() {
        let conn = test_connection();
        let read = |conn: &rusqlite::Connection| {
            parse_auto_check(
                repository::get_setting(conn, KEY_AUTO_CHECK)
                    .unwrap()
                    .as_deref(),
            )
        };
        assert!(!read(&conn), "fresh install must not check automatically");
        repository::set_setting(&conn, KEY_AUTO_CHECK, "true", 1).unwrap();
        assert!(read(&conn));
        repository::set_setting(&conn, KEY_AUTO_CHECK, "false", 2).unwrap();
        assert!(!read(&conn));
    }
}
