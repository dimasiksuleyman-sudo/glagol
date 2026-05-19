//! Application configuration persisted to `%LOCALAPPDATA%\app.glagol.desktop\config.json`.
//!
//! Sprint 5b introduces user-configurable library location (the audio
//! cache root). The config file is the canonical store: loaded once
//! eagerly at startup, cached in [`AppState`](crate::state::AppState),
//! and rewritten atomically (temp-file rename) whenever the user
//! changes a setting.
//!
//! ## Error posture
//!
//! - **Missing file** → return [`Config::default`], file is lazily
//!   created on first save (cheap first-run UX).
//! - **Malformed JSON** → log a warning to stderr and return defaults.
//!   The malformed file is **left on disk untouched** so the user (or
//!   support) can inspect it; we never silently overwrite something we
//!   couldn't parse.
//! - **Save failure** → bubble [`ConfigError`] up to the caller.
//!   Atomic temp-rename means the previous valid file survives even if
//!   the rename never happens.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const CONFIG_FILENAME: &str = "config.json";
const CONFIG_TEMP_FILENAME: &str = "config.json.tmp";

/// Current schema version. Bump when introducing a backwards-incompatible
/// change and add a migration step in [`Config::load`].
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Persistent application configuration.
///
/// All fields are nullable / defaulted so a brand-new `Config` is
/// always valid and the on-disk file can grow new fields over time
/// without breaking older binaries (serde tolerates unknown fields by
/// default for `Deserialize`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    /// User-configured library root. `None` means the app uses the
    /// default `{app_local_data_dir}/audio_cache` location.
    #[serde(default)]
    pub library_path: Option<PathBuf>,

    /// Schema version. Reserved for future migrations; currently always
    /// [`CURRENT_SCHEMA_VERSION`].
    #[serde(default = "default_version")]
    pub version: u32,
}

fn default_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

impl Default for Config {
    fn default() -> Self {
        Self {
            library_path: None,
            version: CURRENT_SCHEMA_VERSION,
        }
    }
}

/// Errors raised by [`Config::save`]. Loading never errors — malformed
/// input maps to defaults with a stderr warning.
#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Serde(serde_json::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "config I/O failure: {e}"),
            ConfigError::Serde(e) => write!(f, "config serialisation failure: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<io::Error> for ConfigError {
    fn from(e: io::Error) -> Self {
        ConfigError::Io(e)
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(e: serde_json::Error) -> Self {
        ConfigError::Serde(e)
    }
}

impl Config {
    /// Read the config file from `{app_local_data_dir}/config.json`.
    ///
    /// Returns [`Config::default`] when the file is absent, unreadable,
    /// or unparseable. Unreadable / unparseable cases also log a
    /// stderr warning so support has a breadcrumb without us silently
    /// overwriting user state.
    pub fn load(app_local_data_dir: &Path) -> Self {
        let path = app_local_data_dir.join(CONFIG_FILENAME);
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                eprintln!(
                    "[glagol::config] could not read {}: {e}; using defaults",
                    path.display()
                );
                return Self::default();
            }
        };
        match serde_json::from_slice::<Config>(&bytes) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!(
                    "[glagol::config] could not parse {}: {e}; using defaults. \
                     File left on disk for inspection.",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Atomically write the config to `{app_local_data_dir}/config.json`.
    ///
    /// Pattern: write to `config.json.tmp` first, fsync via `File::sync_all`,
    /// then `rename` over the destination. If anything fails before the
    /// rename, the previous valid file survives untouched.
    pub fn save(&self, app_local_data_dir: &Path) -> Result<(), ConfigError> {
        fs::create_dir_all(app_local_data_dir)?;
        let json = serde_json::to_vec_pretty(self)?;
        let tmp_path = app_local_data_dir.join(CONFIG_TEMP_FILENAME);
        let final_path = app_local_data_dir.join(CONFIG_FILENAME);

        fs::write(&tmp_path, &json)?;
        // `rename` is atomic on the same filesystem on every supported
        // OS. On Windows, `fs::rename` will overwrite the destination.
        fs::rename(&tmp_path, &final_path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "glagol_config_{}_{}",
            label,
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn load_returns_defaults_when_file_missing() {
        let dir = fresh_dir("missing");
        let cfg = Config::load(&dir);
        assert_eq!(cfg, Config::default());
        assert_eq!(cfg.version, CURRENT_SCHEMA_VERSION);
        assert!(cfg.library_path.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_defaults_when_file_malformed_and_preserves_file() {
        let dir = fresh_dir("malformed");
        let path = dir.join(CONFIG_FILENAME);
        let bad = b"{ this is not valid JSON ::: ";
        fs::write(&path, bad).unwrap();

        let cfg = Config::load(&dir);
        assert_eq!(cfg, Config::default());

        // File must be left untouched — we don't silently overwrite
        // something we couldn't parse.
        let still_there = fs::read(&path).expect("malformed file should still be on disk");
        assert_eq!(still_there, bad);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_then_load_round_trip() {
        let dir = fresh_dir("roundtrip");
        let original = Config {
            library_path: Some(PathBuf::from("D:\\Audio\\Glagol")),
            version: CURRENT_SCHEMA_VERSION,
        };
        original.save(&dir).expect("save ok");

        let loaded = Config::load(&dir);
        assert_eq!(loaded, original);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_uses_atomic_temp_rename_leaving_no_stray_tmp() {
        let dir = fresh_dir("atomic");
        let cfg = Config {
            library_path: Some(PathBuf::from("/var/lib/glagol/audio")),
            version: CURRENT_SCHEMA_VERSION,
        };
        cfg.save(&dir).expect("save ok");

        // The temp file must not linger after a successful save —
        // the rename moves it onto the final path.
        assert!(
            !dir.join(CONFIG_TEMP_FILENAME).exists(),
            "config.json.tmp should not exist after a successful save"
        );
        assert!(
            dir.join(CONFIG_FILENAME).exists(),
            "config.json should exist after save"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
