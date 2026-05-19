//! Path resolution helpers. Single source of truth for filesystem locations.
//!
//! Sprint 5b makes [`audio_cache_root`] respect a user-configured
//! library path stored in [`Config::library_path`](crate::config::Config).
//! Three states are possible at any moment:
//!
//! - **No config / default:** `library_path = None`. Audio lives under
//!   `{app_local_data_dir}/audio_cache/`, unchanged from Sprint 2.
//! - **Configured custom path:** `library_path = Some(path)`. New
//!   synthesis writes go there.
//! - **Mid-migration:** files synthesised before the user changed the
//!   library path still live in the default directory. [`resolve_audio_path`]
//!   transparently falls back to the default root on a per-file basis
//!   so old documents keep playing — Sprint 5b deliberately ships
//!   **without** an auto-move step (D2 in the kickoff Q&A).

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::state::AppState;

const AUDIO_CACHE_DIRNAME: &str = "audio_cache";
const DB_FILENAME: &str = "glagol.db";

/// Effective root directory for synthesized audio files.
///
/// Reads the live [`Config`](crate::config::Config) out of the managed
/// [`AppState`]: if the user has configured a custom library path it
/// is returned verbatim; otherwise the default
/// `{app_local_data_dir}/audio_cache/` is returned.
///
/// Panics if [`AppState`] is not yet managed — that would mean we're
/// calling path resolution before the setup hook has run, which is a
/// programming error rather than a runtime condition.
pub fn audio_cache_root(app: &AppHandle) -> Result<PathBuf, String> {
    let default = default_audio_cache_root(app)?;
    let configured = read_configured_library_path(app);
    Ok(audio_cache_root_impl(default, configured))
}

/// Absolute path to the SQLite database file. Database location stays
/// under `app_local_data_dir` and is **not** configurable — the DB is
/// app metadata, not user content.
pub fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_local_data_dir(app)?.join(DB_FILENAME))
}

/// Resolve a relative audio path (as stored in `documents.audio_path`)
/// to an absolute filesystem path, with **dual-root fallback** when a
/// custom library path is configured:
///
/// 1. Try the configured root — if the file exists there, return it.
/// 2. Otherwise try the default root — if the file exists there
///    (legacy synthesis from before the library was relocated), return
///    that path so the document still plays.
/// 3. Neither exists → return the configured-rooted path. Caller
///    (HTML5 `<audio onerror>`, backend `export_audio`, etc.) handles
///    the not-found condition.
///
/// Collision semantics (impossibly rare — UUIDs are unique): if both
/// roots contain the same file, the **configured root wins** (it is
/// where new synthesis lands).
///
/// When no custom library path is configured, this collapses to plain
/// `{default_root}/{relative}` with zero overhead.
pub fn resolve_audio_path(app: &AppHandle, relative: &str) -> Result<PathBuf, String> {
    let default = default_audio_cache_root(app)?;
    let configured = read_configured_library_path(app);
    Ok(resolve_audio_path_impl(
        &default,
        configured.as_deref(),
        relative,
    ))
}

// ── Internals ──────────────────────────────────────────────────────────

fn app_local_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to resolve app_local_data_dir: {e}"))
}

fn default_audio_cache_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_local_data_dir(app)?.join(AUDIO_CACHE_DIRNAME))
}

/// Snapshot the configured `library_path` out of [`AppState`].
///
/// Acquires the config mutex briefly, clones the `Option<PathBuf>`,
/// releases the guard. The clone is intentional — callers operate on
/// the path without holding the lock.
fn read_configured_library_path(app: &AppHandle) -> Option<PathBuf> {
    let state = app
        .try_state::<AppState>()
        .expect("AppState must be managed before paths:: helpers are called");
    let guard = state.config.lock().expect("config mutex poisoned");
    guard.library_path.clone()
}

fn audio_cache_root_impl(default: PathBuf, configured: Option<PathBuf>) -> PathBuf {
    configured.unwrap_or(default)
}

fn resolve_audio_path_impl(default: &Path, configured: Option<&Path>, relative: &str) -> PathBuf {
    match configured {
        // No custom library — plain default behaviour, zero overhead.
        None => default.join(relative),
        Some(custom) => {
            let custom_full = custom.join(relative);
            if custom_full.exists() {
                // Configured root has the file (incl. collision case —
                // both roots have it, configured wins as per D2).
                return custom_full;
            }
            // File missing at configured — legacy-synthesis fallback.
            let default_full = default.join(relative);
            if default_full.exists() {
                return default_full;
            }
            // Neither root has the file. Return configured-rooted so the
            // caller's not-found error message points at the right place
            // (the place new synthesis would have written to).
            custom_full
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fresh_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "glagol_paths_{}_{}",
            label,
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn audio_cache_root_impl_returns_configured_when_present() {
        let default = PathBuf::from("/default/audio_cache");
        let configured_path = PathBuf::from("D:\\Audio\\Glagol");
        let resolved = audio_cache_root_impl(default, Some(configured_path.clone()));
        assert_eq!(resolved, configured_path);
    }

    #[test]
    fn audio_cache_root_impl_returns_default_when_no_configured() {
        let default = PathBuf::from("/default/audio_cache");
        let resolved = audio_cache_root_impl(default.clone(), None);
        assert_eq!(resolved, default);
    }

    #[test]
    fn resolve_audio_path_impl_dual_root_fallback_and_collision() {
        // Three sub-scenarios in one test (per kickoff: "Edge cases worth
        // covering inline в test names — не обязательно отдельные tests,
        // но в test bodies"):
        //
        //   A. file exists ONLY at default (legacy synthesis)
        //      → fallback to default-rooted path
        //   B. file exists at BOTH (collision, impossibly rare)
        //      → configured wins per D2
        //   C. file exists at NEITHER
        //      → configured-rooted returned (caller handles not-found)

        let default_root = fresh_dir("resolve_default");
        let configured_root = fresh_dir("resolve_configured");

        let relative_a = "legacy-doc.wav";
        let relative_b = "collision-doc.wav";
        let relative_c = "missing-doc.wav";

        // (A) Seed only the default root.
        fs::write(default_root.join(relative_a), b"legacy bytes").unwrap();

        // (B) Seed both roots with the same name; bytes differ so we can
        //     prove which root the resolver picked.
        fs::write(default_root.join(relative_b), b"default bytes").unwrap();
        fs::write(configured_root.join(relative_b), b"configured bytes").unwrap();

        // (C) Seed nothing for `relative_c`.

        // (A) — fall back to default-rooted.
        let a_path = resolve_audio_path_impl(&default_root, Some(&configured_root), relative_a);
        assert_eq!(a_path, default_root.join(relative_a));
        assert!(a_path.exists(), "scenario A path must point at a real file");

        // (B) — configured wins on collision.
        let b_path = resolve_audio_path_impl(&default_root, Some(&configured_root), relative_b);
        assert_eq!(b_path, configured_root.join(relative_b));
        let b_bytes = fs::read(&b_path).unwrap();
        assert_eq!(b_bytes, b"configured bytes");

        // (C) — neither has it; configured-rooted returned for the
        // downstream not-found error to point at the right place.
        let c_path = resolve_audio_path_impl(&default_root, Some(&configured_root), relative_c);
        assert_eq!(c_path, configured_root.join(relative_c));
        assert!(!c_path.exists());

        let _ = fs::remove_dir_all(&default_root);
        let _ = fs::remove_dir_all(&configured_root);
    }
}
