//! Internal error type for the backup/restore subsystem.
//!
//! Mapped to `String` at the Tauri command boundary (see
//! `commands::backup`) per the project-wide convention. Internal call
//! sites keep the structured form so downstream code can branch on the
//! variant — restore's pre-flight validation, for example, distinguishes
//! "this isn't a Glagol backup" (`ValidationFailed`) from "Glagol can't
//! read this backup version" (`VersionUnsupported`) to surface different
//! UI messages.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("Ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),

    #[error("Ошибка при работе с архивом: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Не удалось прочитать manifest.json: {0}")]
    Json(#[from] serde_json::Error),

    /// The zip is structurally valid but does not look like a Glagol
    /// backup (missing `manifest.json`, mismatched counts, unexpected
    /// path traversal, etc.). Human-readable Russian-language reason
    /// suitable for direct toast display.
    #[error("{0}")]
    ValidationFailed(String),

    /// The backup was produced by a newer Glagol than the one trying to
    /// restore it. We refuse rather than guess at forward compatibility.
    #[error("Эта резервная копия создана более новой версией Glagol (формат {found}, поддерживается {supported}). Обновите приложение и попробуйте снова.")]
    VersionUnsupported { found: u32, supported: u32 },

    /// The automatic pre-restore safety backup failed before any
    /// destructive operation took place — caller must abort the restore
    /// with the user's data untouched.
    #[error(
        "Не удалось создать резервную копию текущего состояния перед восстановлением: {reason}"
    )]
    PreRestoreBackupFailed { reason: String },
}

pub type BackupResult<T> = Result<T, BackupError>;
