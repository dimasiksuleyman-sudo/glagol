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

use rusqlite::Connection;
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

    // Swap the real `Connection` out of `AppState` for an in-memory
    // placeholder. Releasing the `Mutex` guard alone is not enough on
    // Windows: the underlying SQLite file handle is owned by the
    // `Connection` value, and `fs::remove_file(glagol.db)` returns
    // `ERROR_SHARING_VIOLATION` (os error 32) while any handle is
    // still open — even though SQLite uses `FILE_SHARE_DELETE`. Taking
    // the `Connection` out by `mem::replace` and dropping it
    // explicitly closes the handle before the destructive work
    // starts.
    //
    // On success, `app.restart()` replaces the process and the
    // placeholder is forgotten. On failure, [`try_restore_real_connection`]
    // best-effort swaps a fresh `Connection` back so the user can keep
    // using the app from the original (un-destroyed) data without a
    // forced restart.
    let placeholder = Connection::open_in_memory()
        .map_err(|e| format!("Не удалось подготовить временное соединение с базой данных: {e}"))?;
    let real_conn = {
        let state = app.state::<AppState>();
        let mut guard = state
            .db
            .lock()
            .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
        std::mem::replace(&mut *guard, placeholder)
    };
    // Explicit drop documents intent: this is *the* moment the SQLite
    // file handle closes. Without the explicit drop the compiler is
    // still free to keep `real_conn` alive until the end of the
    // function — which is exactly the bug we're fixing.
    drop(real_conn);

    let emit_handle = app.clone();
    let restore_result: Result<(), String> = tauri::async_runtime::spawn_blocking(move || {
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

    if restore_result.is_err() {
        try_restore_real_connection(&app);
    }

    restore_result
}

/// Best-effort recovery after a failed restore. Walks `app_local_data_dir`
/// and, if `glagol.db` is still present (the failure happened before the
/// wipe step destroyed it), opens a fresh `Connection` and swaps it back
/// into `AppState` so the user can keep using the app without a forced
/// restart.
///
/// Any step that fails along the way leaves the placeholder in place —
/// the error toast already informed the user the restore did not
/// complete, and their next manual restart picks up a fresh real
/// connection via the normal `setup()` hook. We deliberately do **not**
/// panic here; degraded-state is a better failure mode than crashing
/// the app.
fn try_restore_real_connection(app: &AppHandle) {
    let Ok(data_dir) = app.path().app_local_data_dir() else {
        return;
    };
    let db_path = data_dir.join("glagol.db");
    if !db_path.exists() {
        // The restore wiped the file before failing on the extract
        // step. There is no original DB to reopen; the placeholder
        // stays until the user restarts.
        return;
    }
    let Ok(conn) = Connection::open(&db_path) else {
        return;
    };
    if let Ok(mut guard) = app.state::<AppState>().db.lock() {
        let _placeholder = std::mem::replace(&mut *guard, conn);
        // _placeholder (the in-memory throwaway) drops at end of scope.
    }
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
