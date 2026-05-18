//! Storage-adjacent Tauri commands: resolving the absolute path of a
//! cached audio file and exporting that file to a user-chosen location.
//!
//! Each command is a thin wrapper over an `*_impl` function that takes
//! the explicit dependencies (`&Connection`, audio-root `&Path`). The
//! command wrapper extracts those from `AppState` + `AppHandle`; tests
//! drive the impls directly because `tauri::State` and `AppHandle`
//! cannot be constructed outside a running Tauri runtime.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tauri::{AppHandle, State};

use crate::db;
use crate::paths;
use crate::state::AppState;

/// Resolve the absolute filesystem path of a document's cached audio.
///
/// Returns the path as a `String` (rather than `PathBuf`) because the
/// frontend will feed it straight into the asset protocol once Library
/// playback lands (PR #17). Errors if the row does not exist or its
/// `audio_path` column is `NULL` (Sprint 4 status='error' rows).
#[tauri::command]
pub async fn get_audio_path(
    state: State<'_, AppState>,
    app: AppHandle,
    document_id: String,
) -> Result<String, String> {
    let audio_root = paths::audio_cache_root(&app)?;
    let conn = state.db.lock().expect("db mutex poisoned");
    get_audio_path_impl(&conn, &audio_root, &document_id)
}

/// Copy a document's cached audio to `dest_path` (typically chosen by
/// the user via `dialog.save()`). Source path comes from the DB row;
/// destination is taken at face value.
#[tauri::command]
pub async fn export_audio(
    state: State<'_, AppState>,
    app: AppHandle,
    document_id: String,
    dest_path: String,
) -> Result<(), String> {
    let audio_root = paths::audio_cache_root(&app)?;
    let conn = state.db.lock().expect("db mutex poisoned");
    export_audio_impl(&conn, &audio_root, &document_id, Path::new(&dest_path))
}

pub(crate) fn get_audio_path_impl(
    conn: &Connection,
    audio_root: &Path,
    document_id: &str,
) -> Result<String, String> {
    let absolute = resolve_audio_absolute(conn, audio_root, document_id)?;
    Ok(absolute.to_string_lossy().into_owned())
}

pub(crate) fn export_audio_impl(
    conn: &Connection,
    audio_root: &Path,
    document_id: &str,
    dest_path: &Path,
) -> Result<(), String> {
    let source = resolve_audio_absolute(conn, audio_root, document_id)?;
    fs::copy(&source, dest_path)
        .map_err(|e| format!("failed to copy audio to {}: {e}", dest_path.display()))?;
    Ok(())
}

fn resolve_audio_absolute(
    conn: &Connection,
    audio_root: &Path,
    document_id: &str,
) -> Result<PathBuf, String> {
    let record = db::repository::get(conn, document_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("document not found: {document_id}"))?;
    let relative = record
        .audio_path
        .ok_or_else(|| format!("document has no audio: {document_id}"))?;
    Ok(audio_root.join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::{self, DocumentRecord};
    use crate::db::test_connection;

    fn make_record(id: &str, audio_path: Option<String>) -> DocumentRecord {
        DocumentRecord {
            id: id.to_string(),
            title: "Тест".to_string(),
            source_type: "paste".to_string(),
            char_count: 5,
            voice: "Nec_24000".to_string(),
            status: "ready".to_string(),
            error_message: None,
            created_at: 1_700_000_000_000,
            audio_path,
            audio_duration_ms: None,
        }
    }

    fn unique_tmp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "glagol_storage_{}_{}",
            label,
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn get_audio_path_returns_absolute_path_for_existing_document() {
        let conn = test_connection();
        let audio_root = unique_tmp_dir("get_ok");
        let id = "doc-1";
        let relative = format!("{id}.wav");
        repository::insert(&conn, &make_record(id, Some(relative.clone()))).unwrap();

        let resolved = get_audio_path_impl(&conn, &audio_root, id).expect("resolve ok");
        let expected = audio_root.join(&relative);
        assert_eq!(resolved, expected.to_string_lossy());

        let _ = fs::remove_dir_all(&audio_root);
    }

    #[test]
    fn get_audio_path_returns_error_for_unknown_document() {
        let conn = test_connection();
        let audio_root = unique_tmp_dir("get_missing");

        let err = get_audio_path_impl(&conn, &audio_root, "nope").unwrap_err();
        assert!(
            err.contains("document not found"),
            "expected not-found error, got: {err}"
        );

        let _ = fs::remove_dir_all(&audio_root);
    }

    #[test]
    fn get_audio_path_returns_error_when_audio_path_is_none() {
        let conn = test_connection();
        let audio_root = unique_tmp_dir("get_no_audio");
        repository::insert(&conn, &make_record("err-row", None)).unwrap();

        let err = get_audio_path_impl(&conn, &audio_root, "err-row").unwrap_err();
        assert!(
            err.contains("has no audio"),
            "expected no-audio error, got: {err}"
        );

        let _ = fs::remove_dir_all(&audio_root);
    }

    #[test]
    fn export_audio_copies_file_to_destination() {
        let conn = test_connection();
        let audio_root = unique_tmp_dir("export_ok");
        let id = "doc-export";
        let relative = format!("{id}.wav");
        let source = audio_root.join(&relative);
        let payload: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
        fs::write(&source, &payload).expect("seed source file");
        repository::insert(&conn, &make_record(id, Some(relative))).unwrap();

        let dest_dir = unique_tmp_dir("export_dest");
        let dest = dest_dir.join("out.wav");

        export_audio_impl(&conn, &audio_root, id, &dest).expect("export ok");
        let read_back = fs::read(&dest).expect("read back dest");
        assert_eq!(read_back, payload);

        let _ = fs::remove_dir_all(&audio_root);
        let _ = fs::remove_dir_all(&dest_dir);
    }

    #[test]
    fn export_audio_returns_error_for_unknown_document() {
        let conn = test_connection();
        let audio_root = unique_tmp_dir("export_missing");
        let dest_dir = unique_tmp_dir("export_missing_dest");
        let dest = dest_dir.join("out.wav");

        let err = export_audio_impl(&conn, &audio_root, "ghost", &dest).unwrap_err();
        assert!(
            err.contains("document not found"),
            "expected not-found error, got: {err}"
        );

        let _ = fs::remove_dir_all(&audio_root);
        let _ = fs::remove_dir_all(&dest_dir);
    }
}
