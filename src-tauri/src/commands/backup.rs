//! Tauri commands for the backup/restore feature (Sprint 5c).
//!
//! Each user-facing operation is a thin async wrapper that resolves
//! filesystem roots from the Tauri `AppHandle`, then hands off to the
//! pure `*_impl` function inside `crate::backup` via
//! `tauri::async_runtime::spawn_blocking` — the zip work is sync and
//! fs-heavy, and parking it on the blocking pool keeps the Tokio
//! reactor free for the next IPC call.
//!
//! Progress is broadcast via `app.emit(...)`. Two separate channels
//! exist — [`BACKUP_PROGRESS_EVENT`] for the «Создаю резервную копию»
//! flow and [`BACKUP_RESTORE_PROGRESS_EVENT`] for «Восстанавливаю» —
//! so the frontend can subscribe with `listen()` on the channel that
//! matches the modal currently open, with no risk of crosstalk.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::backup::create::create_backup_impl;
use crate::backup::restore::{restore_backup_impl, validate_backup_impl};
use crate::backup::{BackupManifest, BACKUP_FILENAME_PREFIX};
use crate::state::AppState;

/// Event payload shared between backup and restore. `current` and
/// `total` are file counts, not byte counts — the modal renders
/// "Создаю резервную копию... ({current}/{total} файлов)".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupProgressEvent {
    pub current: u64,
    pub total: u64,
}

/// Channel name for backup creation progress.
pub const BACKUP_PROGRESS_EVENT: &str = "backup-progress";

/// Channel name for restore progress. Kept distinct from
/// [`BACKUP_PROGRESS_EVENT`] so the two modals can each `listen()` to
/// exactly their own stream and ignore the other operation entirely.
pub const BACKUP_RESTORE_PROGRESS_EVENT: &str = "backup-restore-progress";

#[tauri::command]
pub async fn create_backup(app: AppHandle, target_folder: String) -> Result<String, String> {
    let source_data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Не удалось определить папку данных приложения: {e}"))?;
    let target_path = PathBuf::from(&target_folder);
    let app_version = env!("CARGO_PKG_VERSION").to_string();

    let emit_handle = app.clone();
    let result: Result<PathBuf, String> = tauri::async_runtime::spawn_blocking(move || {
        create_backup_impl(
            &source_data_dir,
            &target_path,
            &app_version,
            BACKUP_FILENAME_PREFIX,
            |current, total| {
                let _ = emit_handle.emit(
                    BACKUP_PROGRESS_EVENT,
                    BackupProgressEvent { current, total },
                );
            },
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Задача создания резервной копии прервалась: {e}"))?;

    result.map(|path| path.to_string_lossy().into_owned())
}

/// Pre-flight check for a candidate restore source. Reads the
/// manifest, runs the validation chain, and hands the manifest back
/// to the frontend so it can render the confirm dialog with the
/// document count baked into the question. Cheap (~50 ms) and never
/// touches user data — safe to call from any UI flow.
#[tauri::command]
pub async fn validate_backup(source_path: String) -> Result<BackupManifest, String> {
    let path = PathBuf::from(source_path);
    tauri::async_runtime::spawn_blocking(move || {
        validate_backup_impl(&path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Проверка резервной копии прервалась: {e}"))?
}

#[tauri::command]
pub async fn restore_backup(app: AppHandle, source_path: String) -> Result<(), String> {
    let source_zip = PathBuf::from(source_path);
    let target_data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Не удалось определить папку данных приложения: {e}"))?;
    let app_version = env!("CARGO_PKG_VERSION").to_string();

    // Release the AppState's SQLite connection before we delete the
    // database file. We acquire the lock, then immediately drop it —
    // that forces any pending borrow to finish, and because Sprint 5c
    // does *not* hot-reload the connection after restore (the app
    // restarts on success), there is no need to put anything back in
    // its place. The next `glagol.db` open happens on the relaunched
    // process via the setup hook.
    {
        let state = app.state::<AppState>();
        let _guard = state
            .db
            .lock()
            .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
        // _guard drops at the closing brace, releasing the Mutex.
    }

    let emit_handle = app.clone();
    let result: Result<(), String> = tauri::async_runtime::spawn_blocking(move || {
        restore_backup_impl(
            &source_zip,
            &target_data_dir,
            &app_version,
            |current, total| {
                let _ = emit_handle.emit(
                    BACKUP_RESTORE_PROGRESS_EVENT,
                    BackupProgressEvent { current, total },
                );
            },
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Задача восстановления прервалась: {e}"))?;

    result
}

/// Restart the application — used after a successful restore to pick
/// up the freshly extracted `glagol.db` on a clean Tauri setup hook.
///
/// Implemented as a sync `fn` because `AppHandle::restart` returns
/// `!` (process is replaced before control returns), so wrapping it
/// in `async` would only add ceremony.
#[tauri::command]
pub fn relaunch_app(app: AppHandle) {
    app.restart();
}
