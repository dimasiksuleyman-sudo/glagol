//! Tauri commands for the backup/restore feature (Sprint 5c).
//!
//! Each user-facing operation is a thin async wrapper that resolves
//! filesystem roots from the Tauri `AppHandle`, then hands off to the
//! pure `*_impl` function inside `crate::backup` via
//! `tauri::async_runtime::spawn_blocking` — the zip work is sync and
//! fs-heavy, and parking it on the blocking pool keeps the Tokio
//! reactor free for the next IPC call.
//!
//! Progress is broadcast via `app.emit("backup-progress", …)`. The
//! frontend `listen()`s for the duration of the modal and stops as
//! soon as the command resolves. We chose `emit` over the
//! `tauri::ipc::Channel` pattern used by `synthesize_document` because
//! the modal lifecycle is naturally one-shot (no multiplexing across
//! concurrent backups, only one button can be pressed at a time).

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::backup::create::create_backup_impl;

/// Event broadcast on the `backup-progress` channel while a backup or
/// restore operation is in flight.
///
/// `current` and `total` are file counts, not byte counts — the modal
/// renders "Создаю резервную копию... ({current}/{total} файлов)".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupProgressEvent {
    pub current: u64,
    pub total: u64,
}

/// Channel name shared with the frontend listener. Single source of
/// truth so future renames stay in lock-step.
pub const BACKUP_PROGRESS_EVENT: &str = "backup-progress";

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
