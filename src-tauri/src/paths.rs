//! Path resolution helpers. Single source of truth for filesystem locations.
//!
//! Sprint 2: hardcoded defaults via Tauri's `app_local_data_dir`.
//! Sprint 5c (planned): `audio_cache_root` becomes configurable via the
//! Settings page. When that change lands, this module is the only place
//! that needs to learn about the user-configured root — call sites stay
//! unchanged.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// Root directory for synthesized audio files.
///
/// Default: `%LOCALAPPDATA%\Glagol\audio_cache\` on Windows, the platform
/// equivalent elsewhere. The directory is not created here — callers that
/// need to write to it must `std::fs::create_dir_all` themselves.
pub fn audio_cache_root(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to resolve app_local_data_dir: {e}"))?;
    Ok(base.join("audio_cache"))
}

/// Absolute path to the SQLite database file.
///
/// Default: `%LOCALAPPDATA%\Glagol\glagol.db`. The parent directory is
/// created by [`crate::db::init_database`] before opening the connection.
pub fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to resolve app_local_data_dir: {e}"))?;
    Ok(base.join("glagol.db"))
}

/// Resolve a relative audio path (as stored in the `documents.audio_path`
/// column) to an absolute filesystem path under [`audio_cache_root`].
pub fn resolve_audio_path(app: &AppHandle, relative: &str) -> Result<PathBuf, String> {
    Ok(audio_cache_root(app)?.join(relative))
}
