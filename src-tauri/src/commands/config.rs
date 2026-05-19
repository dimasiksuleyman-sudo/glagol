//! Tauri commands backing the Settings → "Папка библиотеки" section.
//!
//! Two commands form the surface:
//!
//! * [`get_library_path`] — returns both the **raw** configured value
//!   (so the UI can decide whether to show the "Сбросить" button) and
//!   the **effective** path actually in use right now.
//! * [`set_library_path`] — validates a user-picked path through the
//!   D4 chain (absolute → create_dir_all → test-write → canonicalise
//!   → compare with default → persist → register asset-protocol
//!   scope). An empty string is the **reset** signal — equivalent to
//!   storing `library_path: None`.
//!
//! Validation outcomes are returned as Russian-language string errors
//! suitable for direct display in a toast on the Settings page.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::config::Config;
use crate::paths;
use crate::state::AppState;

const WRITABLE_PROBE_FILENAME: &str = ".glagol_writable_check.tmp";

/// Snapshot returned to the UI by [`get_library_path`].
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct LibraryPathInfo {
    /// The raw value stored in `config.json` — `None` means "use the
    /// default location". The UI uses `configured.is_some()` to show
    /// or hide the "Сбросить" button.
    pub configured: Option<String>,
    /// The path actually in use right now. When `configured` is `None`,
    /// this is `{app_local_data_dir}/audio_cache`; when `configured`
    /// is `Some(path)`, this is `path`.
    pub effective: String,
}

#[tauri::command]
pub async fn get_library_path(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<LibraryPathInfo, String> {
    let data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to resolve app_local_data_dir: {e}"))?;
    let default_root = paths::default_audio_cache_root_under(&data_dir);
    Ok(get_library_path_info(&state, &default_root))
}

#[tauri::command]
pub async fn set_library_path(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
) -> Result<(), String> {
    let data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to resolve app_local_data_dir: {e}"))?;
    let default_root = paths::default_audio_cache_root_under(&data_dir);

    set_library_path_with_paths(&state, &data_dir, &default_root, path)?;

    // After save, register the dynamic asset-protocol scope for the
    // final library_path if it is `Some(...)`. The set_library_path_with_paths
    // helper may have canonicalised an input matching the default down
    // to `None`, so we re-read state to know the actual final value.
    let final_path = state
        .config
        .lock()
        .expect("config mutex poisoned")
        .library_path
        .clone();
    if let Some(custom) = final_path {
        app.asset_protocol_scope()
            .allow_directory(&custom, false)
            .map_err(|e| {
                format!(
                    "Папка сохранена, но не удалось зарегистрировать её для воспроизведения: {e}"
                )
            })?;
    }
    Ok(())
}

// ── Pure helpers (testable without AppHandle) ────────────────────────

pub(crate) fn get_library_path_info(state: &AppState, default_root: &Path) -> LibraryPathInfo {
    let guard = state.config.lock().expect("config mutex poisoned");
    let configured = guard.library_path.as_ref().map(|p| p.display().to_string());
    let effective = guard
        .library_path
        .clone()
        .unwrap_or_else(|| default_root.to_path_buf());
    LibraryPathInfo {
        configured,
        effective: effective.display().to_string(),
    }
}

/// D4 validation + persistence chain. Mutates `state.config` and
/// flushes to disk inside `data_dir`. Does NOT touch the asset-protocol
/// scope — that is the caller's job (it needs the live `AppHandle`).
pub(crate) fn set_library_path_with_paths(
    state: &AppState,
    data_dir: &Path,
    default_root: &Path,
    path: String,
) -> Result<(), String> {
    // (1) Empty string = explicit reset to default. No path operations.
    if path.is_empty() {
        return commit_library_path(state, data_dir, None);
    }

    // (2) Absolute path required (defence-in-depth; the dialog plugin
    // always returns absolute paths, but the IPC boundary is untrusted).
    let candidate = PathBuf::from(&path);
    if !candidate.is_absolute() {
        return Err("Путь должен быть абсолютным.".to_string());
    }

    // (3) Ensure the directory exists. No-op if it already does.
    fs::create_dir_all(&candidate).map_err(|e| format!("Не удалось создать папку: {e}"))?;

    // (4) Writable probe — create + remove a small marker file. The
    // remove is best-effort; if it leaks the user will see a stray
    // `.glagol_writable_check.tmp`, which is harmless.
    let probe = candidate.join(WRITABLE_PROBE_FILENAME);
    fs::write(&probe, b"").map_err(|e| format!("Папка недоступна для записи: {e}"))?;
    let _ = fs::remove_file(&probe);

    // (5) Canonicalise so we store one stable representation regardless
    // of how the user wrote the path (forward / back slashes, trailing
    // separator, relative symlinks). If the canonical result equals
    // the canonical default, store `None` as the semantic equivalent.
    //
    // `dunce::canonicalize` instead of `std::fs::canonicalize` because
    // on Windows the std version unconditionally returns the extended-
    // length prefix form (`\\?\D:\foo`). That prefix contaminates every
    // downstream consumer: Tauri's asset-protocol scope glob doesn't
    // match prefixed paths, dual-root fallback comparison fails,
    // `fs::write` to a prefixed path lands inconsistently across
    // Windows API layers. `dunce` returns the clean `D:\foo` form
    // when the path doesn't actually need the prefix (i.e. fits
    // MAX_PATH and is valid Win32 syntax). On non-Windows it
    // delegates to `std::fs::canonicalize` unchanged.
    let canonical =
        dunce::canonicalize(&candidate).map_err(|e| format!("Не удалось разрешить путь: {e}"))?;

    // The default audio-cache directory may not exist yet on a first
    // launch — `create_dir_all` here so canonicalize won't fail. Mirrors
    // what `setup()` does for the default path at startup.
    fs::create_dir_all(default_root)
        .map_err(|e| format!("Не удалось подготовить путь по умолчанию: {e}"))?;
    let canonical_default = dunce::canonicalize(default_root)
        .map_err(|e| format!("Не удалось разрешить путь по умолчанию: {e}"))?;

    let final_value = if canonical == canonical_default {
        None
    } else {
        Some(canonical)
    };
    commit_library_path(state, data_dir, final_value)
}

/// Mutate the in-memory config + flush to disk atomically. If the disk
/// write fails the in-memory value is reverted so disk + memory stay
/// consistent.
fn commit_library_path(
    state: &AppState,
    data_dir: &Path,
    new_value: Option<PathBuf>,
) -> Result<(), String> {
    let mut guard = state.config.lock().expect("config mutex poisoned");
    let prior = guard.library_path.clone();
    guard.library_path = new_value;
    let snapshot: Config = guard.clone();
    if let Err(e) = snapshot.save(data_dir) {
        guard.library_path = prior;
        return Err(format!("Не удалось сохранить настройки: {e}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::sync::Mutex;

    // We construct a minimal AppState for tests by hand. The full
    // `AppState::new` requires a reqwest::Client + Connection that we
    // don't need here — only `config: Mutex<Config>` is consulted by
    // these helpers.
    fn state_with_config(cfg: Config) -> AppState {
        let client = crate::salute::http::build_client().expect("client builds");
        let conn = crate::db::test_connection();
        AppState {
            http_client: client,
            salute_auth: tokio::sync::Mutex::new(None),
            db: Mutex::new(conn),
            config: Mutex::new(cfg),
        }
    }

    fn fresh_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "glagol_cmd_config_{}_{}",
            label,
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn get_library_path_info_pure_logic_for_both_branches() {
        let default_root = PathBuf::from("/var/glagol/audio_cache");

        // Branch 1: nothing configured → effective = default,
        // configured = None.
        let unset = state_with_config(Config::default());
        let info = get_library_path_info(&unset, &default_root);
        assert_eq!(info.configured, None);
        assert_eq!(info.effective, default_root.display().to_string());

        // Branch 2: custom path configured → both fields point at it.
        let custom = PathBuf::from("/mnt/audio/glagol");
        let set = state_with_config(Config {
            library_path: Some(custom.clone()),
            version: crate::config::CURRENT_SCHEMA_VERSION,
        });
        let info = get_library_path_info(&set, &default_root);
        assert_eq!(info.configured, Some(custom.display().to_string()));
        assert_eq!(info.effective, custom.display().to_string());
    }

    #[test]
    fn set_library_path_with_paths_full_validation_chain() {
        // Five sub-scenarios bundled per kickoff "Edge cases worth
        // covering inline в test names — не обязательно отдельные tests,
        // но в test bodies":
        //
        //   A. happy path with absolute writable temp dir → persists
        //   B. re-saving the same path is idempotent (no error)
        //   C. empty string → resets library_path to None
        //   D. non-absolute path → Russian-language error, config unchanged
        //   E. path that canonicalises to the default → stored as None

        let data_dir = fresh_dir("data");
        let default_root = paths::default_audio_cache_root_under(&data_dir);
        // setup-hook normally creates this; mirror that for the tests.
        fs::create_dir_all(&default_root).unwrap();

        let state = state_with_config(Config::default());
        let custom_dir = fresh_dir("custom");

        // (A) Happy path.
        let path_a = custom_dir.to_string_lossy().into_owned();
        set_library_path_with_paths(&state, &data_dir, &default_root, path_a.clone())
            .expect("happy path saves");
        {
            let cfg = state.config.lock().unwrap();
            assert!(cfg.library_path.is_some());
            // Compare against `dunce::canonicalize` — the same primitive
            // production uses — so the test stays apples-to-apples on
            // Windows where `std::fs::canonicalize` would still return
            // the `\\?\` extended-length form.
            assert_eq!(
                cfg.library_path.as_ref().unwrap(),
                &dunce::canonicalize(&custom_dir).unwrap()
            );
        }
        assert!(
            data_dir.join("config.json").exists(),
            "config.json must be written to disk"
        );

        // (B) Re-saving the same path = no-op success.
        set_library_path_with_paths(&state, &data_dir, &default_root, path_a)
            .expect("idempotent re-save");

        // (C) Empty string = reset to default.
        set_library_path_with_paths(&state, &data_dir, &default_root, String::new())
            .expect("empty resets");
        assert!(state.config.lock().unwrap().library_path.is_none());

        // (D) Non-absolute path is rejected with a Russian-language
        // error; nothing was persisted.
        let pre_d = state.config.lock().unwrap().library_path.clone();
        let err = set_library_path_with_paths(
            &state,
            &data_dir,
            &default_root,
            "relative/path".to_string(),
        )
        .unwrap_err();
        assert!(
            err.contains("абсолютным"),
            "expected absolute-path error in Russian, got: {err}"
        );
        assert_eq!(state.config.lock().unwrap().library_path, pre_d);

        // (E) Saving the canonical default itself collapses to None.
        let default_str = default_root.to_string_lossy().into_owned();
        set_library_path_with_paths(&state, &data_dir, &default_root, default_str)
            .expect("saving default canonicalises to None");
        assert!(
            state.config.lock().unwrap().library_path.is_none(),
            "default-path input must collapse to library_path: None"
        );

        let _ = fs::remove_dir_all(&data_dir);
        let _ = fs::remove_dir_all(&custom_dir);
    }

    /// Windows-only regression guard for the Sprint 5b post-merge bug:
    /// `std::fs::canonicalize` returns `\\?\D:\foo` (extended-length
    /// form) which contaminated `Config.library_path` and broke every
    /// downstream consumer (asset-protocol scope, dual-root fallback,
    /// `fs::write` consistency). After the hotfix, `dunce::canonicalize`
    /// produces the clean Win32 form for paths that fit MAX_PATH.
    ///
    /// Linux / macOS don't have this issue — `canonicalize` returns
    /// clean paths unconditionally there — so the test is gated on
    /// `cfg(windows)`.
    #[test]
    #[cfg(windows)]
    fn set_library_path_with_paths_does_not_emit_extended_length_prefix_on_windows() {
        let data_dir = fresh_dir("no_prefix_data");
        let default_root = paths::default_audio_cache_root_under(&data_dir);
        fs::create_dir_all(&default_root).unwrap();

        let custom_dir = fresh_dir("no_prefix_custom");
        let state = state_with_config(Config::default());

        set_library_path_with_paths(
            &state,
            &data_dir,
            &default_root,
            custom_dir.to_string_lossy().into_owned(),
        )
        .expect("save succeeds");

        let stored = state
            .config
            .lock()
            .unwrap()
            .library_path
            .clone()
            .expect("a custom path is stored");
        let stored_str = stored.to_string_lossy();

        assert!(
            !stored_str.starts_with(r"\\?\"),
            "stored library_path must not carry the extended-length prefix, got: {stored_str}"
        );

        let _ = fs::remove_dir_all(&data_dir);
        let _ = fs::remove_dir_all(&custom_dir);
    }
}
