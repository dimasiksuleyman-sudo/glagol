//! Storage-adjacent Tauri commands: resolving the absolute path of a
//! cached audio file, exporting that file to a user-chosen location,
//! listing every persisted document, and deleting a document (row + file).
//!
//! Each command is a thin wrapper over an `*_impl` function that takes
//! the explicit dependencies (`&Connection` or `&Mutex<Connection>`,
//! audio-root `&Path`). The command wrapper extracts those from
//! `AppState` + `AppHandle`; tests drive the impls directly because
//! `tauri::State` and `AppHandle` cannot be constructed outside a
//! running Tauri runtime.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;
use tauri::{AppHandle, State};

use crate::db;
use crate::db::repository::DocumentRecord;
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

/// List every persisted document, most recently created first.
///
/// Thin wrapper over [`db::repository::list_all`]; serde turns the
/// `Vec<DocumentRecord>` into JSON automatically at the IPC boundary.
#[tauri::command]
pub async fn list_documents(state: State<'_, AppState>) -> Result<Vec<DocumentRecord>, String> {
    let conn = state.db.lock().expect("db mutex poisoned");
    list_documents_impl(&conn)
}

pub(crate) fn list_documents_impl(conn: &Connection) -> Result<Vec<DocumentRecord>, String> {
    db::repository::list_all(conn).map_err(|e| e.to_string())
}

/// Delete a document: remove the DB row, then best-effort remove the
/// cached audio file from disk.
///
/// Lock discipline: the mutex guard is dropped before any filesystem
/// operation. AV scanners on Windows can stall `fs::remove_file` for
/// hundreds of milliseconds; holding the DB lock across that would
/// block every concurrent command. An orphaned cache file (DB row
/// gone, file remains) is acceptable — Sprint 5 cleanup will sweep it.
/// A missing file on entry is also acceptable (already-removed orphan,
/// or a row whose `audio_path` was never written).
#[tauri::command]
pub async fn delete_document(
    state: State<'_, AppState>,
    app: AppHandle,
    document_id: String,
) -> Result<(), String> {
    let audio_root = paths::audio_cache_root(&app)?;
    delete_document_impl(&state.db, &audio_root, &document_id)
}

pub(crate) fn delete_document_impl(
    db: &Mutex<Connection>,
    audio_root: &Path,
    document_id: &str,
) -> Result<(), String> {
    let relative_audio = {
        let conn = db.lock().expect("db mutex poisoned");
        let record = db::repository::get(&conn, document_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("document not found: {document_id}"))?;
        db::repository::delete(&conn, document_id).map_err(|e| e.to_string())?;
        record.audio_path
        // Guard drops here at end of scope, before any fs op below.
    };

    if let Some(rel) = relative_audio {
        let abs = audio_root.join(rel);
        // Best-effort: a missing file is fine (orphan or never-written row).
        let _ = fs::remove_file(&abs);
    }
    Ok(())
}

/// Rename a document. Trims the title at the IPC boundary, rejects
/// empty / whitespace-only titles before touching the DB. Returns a
/// Russian-language error string suitable for direct toast display
/// when the document doesn't exist.
#[tauri::command]
pub async fn update_document_title(
    state: State<'_, AppState>,
    document_id: String,
    title: String,
) -> Result<(), String> {
    update_document_title_impl(&state.db, &document_id, &title)
}

pub(crate) fn update_document_title_impl(
    db: &Mutex<Connection>,
    document_id: &str,
    title: &str,
) -> Result<(), String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err("Заголовок не может быть пустым".to_string());
    }
    let conn = db.lock().expect("db mutex poisoned");
    let rows =
        db::repository::update_title(&conn, document_id, trimmed).map_err(|e| e.to_string())?;
    if rows == 0 {
        return Err("Документ не найден".to_string());
    }
    Ok(())
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

    fn make_record_full(id: &str, created_at: i64) -> DocumentRecord {
        DocumentRecord {
            id: id.to_string(),
            title: format!("Документ {id}"),
            source_type: "paste".to_string(),
            char_count: 42,
            voice: "Nec_24000".to_string(),
            status: "ready".to_string(),
            error_message: None,
            created_at,
            audio_path: Some(format!("{id}.wav")),
            audio_duration_ms: Some(12_345),
        }
    }

    #[test]
    fn list_documents_impl_returns_rows_ordered_by_created_at_desc() {
        let conn = test_connection();
        let oldest = make_record_full("a-oldest", 1_000);
        let middle = make_record_full("b-middle", 2_000);
        let newest = make_record_full("c-newest", 3_000);

        repository::insert(&conn, &oldest).unwrap();
        repository::insert(&conn, &newest).unwrap();
        repository::insert(&conn, &middle).unwrap();

        let docs = list_documents_impl(&conn).expect("list ok");
        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0], newest);
        assert_eq!(docs[1], middle);
        assert_eq!(docs[2], oldest);
    }

    #[test]
    fn delete_document_impl_removes_row_and_file() {
        let db = Mutex::new(test_connection());
        let audio_root = unique_tmp_dir("del_ok");
        let id = "doc-to-delete";
        let relative = format!("{id}.wav");
        let abs = audio_root.join(&relative);
        fs::write(&abs, b"some audio").expect("seed file");

        {
            let conn = db.lock().unwrap();
            repository::insert(&conn, &make_record(id, Some(relative))).unwrap();
        }

        delete_document_impl(&db, &audio_root, id).expect("delete ok");

        let conn = db.lock().unwrap();
        assert!(
            repository::get(&conn, id).unwrap().is_none(),
            "row should be gone"
        );
        assert!(!abs.exists(), "file should be gone");

        drop(conn);
        let _ = fs::remove_dir_all(&audio_root);
    }

    #[test]
    fn delete_document_impl_returns_error_for_unknown_id() {
        let db = Mutex::new(test_connection());
        let audio_root = unique_tmp_dir("del_unknown");

        let err = delete_document_impl(&db, &audio_root, "never-existed").unwrap_err();
        assert!(
            err.contains("document not found"),
            "expected not-found error, got: {err}"
        );

        let _ = fs::remove_dir_all(&audio_root);
    }

    #[test]
    fn delete_document_impl_succeeds_when_file_already_missing() {
        let db = Mutex::new(test_connection());
        let audio_root = unique_tmp_dir("del_no_file");
        let id = "orphan-row";
        let relative = format!("{id}.wav");

        {
            let conn = db.lock().unwrap();
            repository::insert(&conn, &make_record(id, Some(relative))).unwrap();
            // Deliberately do NOT write the audio file.
        }

        delete_document_impl(&db, &audio_root, id)
            .expect("delete should succeed even if file is missing");

        let conn = db.lock().unwrap();
        assert!(
            repository::get(&conn, id).unwrap().is_none(),
            "row should still be removed even though file was absent"
        );

        drop(conn);
        let _ = fs::remove_dir_all(&audio_root);
    }

    #[test]
    fn delete_document_impl_releases_lock_before_returning() {
        // The lock must be dropped before the filesystem op so concurrent
        // commands aren't blocked on slow AV scanners. `try_lock()`
        // returning Ok after the impl returns proves no guard is still
        // alive at that point.
        let db = Mutex::new(test_connection());
        let audio_root = unique_tmp_dir("del_lock");
        let id = "lock-test";
        let relative = format!("{id}.wav");
        let abs = audio_root.join(&relative);
        fs::write(&abs, b"x").unwrap();

        {
            let conn = db.lock().unwrap();
            repository::insert(&conn, &make_record(id, Some(relative))).unwrap();
        }

        delete_document_impl(&db, &audio_root, id).expect("delete ok");

        assert!(
            db.try_lock().is_ok(),
            "Mutex must be releasable immediately after delete_document_impl returns"
        );

        let _ = fs::remove_dir_all(&audio_root);
    }

    #[test]
    fn update_document_title_impl_full_validation_chain() {
        // Four inline sub-scenarios per the kickoff "Edge cases worth
        // covering inline в test names — не обязательно отдельные tests,
        // но в test bodies" pattern:
        //
        //   A. happy path with whitespace-padded input → trimmed + saved
        //   B. empty string → Russian-language rejection, DB untouched
        //   C. whitespace-only → same rejection (trim drives it)
        //   D. unknown id → "Документ не найден"

        let db = Mutex::new(test_connection());
        let id = "doc-rename";
        {
            let conn = db.lock().unwrap();
            repository::insert(&conn, &make_record(id, Some("orig.wav".to_string()))).unwrap();
        }

        // (A) Happy + trim: incoming title has padding which the
        // command strips before persisting.
        update_document_title_impl(&db, id, "  Свежий заголовок  ").expect("happy path with trim");
        {
            let conn = db.lock().unwrap();
            let row = repository::get(&conn, id).unwrap().unwrap();
            assert_eq!(row.title, "Свежий заголовок");
        }

        // (B) Empty string → reject; DB unchanged.
        let err_b = update_document_title_impl(&db, id, "").unwrap_err();
        assert!(
            err_b.contains("пустым"),
            "expected empty-title rejection in Russian, got: {err_b}"
        );

        // (C) Whitespace-only → reject (post-trim).
        let err_c = update_document_title_impl(&db, id, "   \t  \n  ").unwrap_err();
        assert!(
            err_c.contains("пустым"),
            "expected whitespace-title rejection in Russian, got: {err_c}"
        );

        // Title from (A) survives the failed (B) and (C) calls.
        {
            let conn = db.lock().unwrap();
            let row = repository::get(&conn, id).unwrap().unwrap();
            assert_eq!(row.title, "Свежий заголовок");
        }

        // (D) Unknown id → not-found.
        let err_d = update_document_title_impl(&db, "ghost-id", "Призрак").unwrap_err();
        assert!(
            err_d.contains("не найден"),
            "expected not-found error in Russian, got: {err_d}"
        );
    }
}
