//! Repository functions for the `documents` table.
//!
//! Pattern: stateless free functions taking `&Connection`, mirroring the
//! conventions of `text::chunker` and `audio::wav_join` from Sprint 1. The
//! repository does not generate IDs or timestamps — callers pass in a fully
//! formed [`DocumentRecord`] so the layer stays deterministic and trivial to
//! unit-test.

use rusqlite::{params, Connection, OptionalExtension, Result, Row};
use serde::{Deserialize, Serialize};

const INSERT_SQL: &str = "
    INSERT INTO documents (
        id, title, source_type, char_count, voice, status,
        error_message, created_at, audio_path, audio_duration_ms
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
";

const SELECT_COLUMNS: &str = "
    id, title, source_type, char_count, voice, status,
    error_message, created_at, audio_path, audio_duration_ms
";

/// Persisted document record. Matches the `documents` table schema 1:1.
///
/// `source_type` and `status` are enum-like strings rather than dedicated
/// types because SQLite has no enum support — keeping them as `String` here
/// avoids a serialization layer for the repository. Callers are expected to
/// use the documented vocabulary (Sprint 2: `source_type="paste"`,
/// `status="ready"`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentRecord {
    pub id: String,
    pub title: String,
    pub source_type: String,
    pub char_count: i64,
    pub voice: String,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: i64,
    pub audio_path: Option<String>,
    pub audio_duration_ms: Option<i64>,
}

impl DocumentRecord {
    fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            title: row.get("title")?,
            source_type: row.get("source_type")?,
            char_count: row.get("char_count")?,
            voice: row.get("voice")?,
            status: row.get("status")?,
            error_message: row.get("error_message")?,
            created_at: row.get("created_at")?,
            audio_path: row.get("audio_path")?,
            audio_duration_ms: row.get("audio_duration_ms")?,
        })
    }
}

/// Insert a new document row. Fails with a UNIQUE constraint error if `id`
/// already exists.
pub fn insert(conn: &Connection, doc: &DocumentRecord) -> Result<()> {
    conn.execute(
        INSERT_SQL,
        params![
            doc.id,
            doc.title,
            doc.source_type,
            doc.char_count,
            doc.voice,
            doc.status,
            doc.error_message,
            doc.created_at,
            doc.audio_path,
            doc.audio_duration_ms,
        ],
    )?;
    Ok(())
}

/// Fetch a single document by primary key.
pub fn get(conn: &Connection, id: &str) -> Result<Option<DocumentRecord>> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM documents WHERE id = ?1");
    conn.query_row(&sql, params![id], DocumentRecord::from_row)
        .optional()
}

/// List every document, most recently created first.
pub fn list_all(conn: &Connection) -> Result<Vec<DocumentRecord>> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM documents ORDER BY created_at DESC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], DocumentRecord::from_row)?;
    rows.collect()
}

/// Delete a document by primary key. Returns the number of rows affected
/// (0 if the id did not exist, 1 otherwise).
pub fn delete(conn: &Connection, id: &str) -> Result<usize> {
    conn.execute("DELETE FROM documents WHERE id = ?1", params![id])
}

/// Update the title of an existing document. Returns the number of
/// rows affected (0 if the id did not exist, 1 otherwise). The caller
/// at the command layer translates `0` into a user-facing
/// "Документ не найден" error — matching the existing `delete`
/// pattern rather than introducing a separate `DbError::NotFound`.
pub fn update_title(conn: &Connection, id: &str, title: &str) -> Result<usize> {
    conn.execute(
        "UPDATE documents SET title = ?2 WHERE id = ?1",
        params![id, title],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_connection;

    fn sample_record(id: &str, created_at: i64) -> DocumentRecord {
        DocumentRecord {
            id: id.to_string(),
            title: "Война и мир".to_string(),
            source_type: "paste".to_string(),
            char_count: 1234,
            voice: "Nec_24000".to_string(),
            status: "ready".to_string(),
            error_message: None,
            created_at,
            audio_path: Some(format!("{id}.wav")),
            audio_duration_ms: Some(42_000),
        }
    }

    #[test]
    fn insert_then_get_returns_same_record() {
        let conn = test_connection();
        let doc = sample_record("a3f2c9d1-7b4e-4f5a-bcde-1234567890ab", 1_700_000_000_000);

        insert(&conn, &doc).expect("insert succeeds");
        let fetched = get(&conn, &doc.id).expect("get succeeds");

        assert_eq!(fetched, Some(doc));
    }

    #[test]
    fn get_returns_none_for_missing_id() {
        let conn = test_connection();
        let fetched = get(&conn, "does-not-exist").expect("get returns Ok");
        assert!(fetched.is_none());
    }

    #[test]
    fn insert_duplicate_id_fails() {
        let conn = test_connection();
        let doc = sample_record("dup-id", 1);
        insert(&conn, &doc).expect("first insert succeeds");

        let err = insert(&conn, &doc).expect_err("second insert should fail");
        assert!(
            matches!(err, rusqlite::Error::SqliteFailure(_, _)),
            "expected SqliteFailure for PRIMARY KEY violation, got {err:?}"
        );
    }

    #[test]
    fn list_all_returns_most_recent_first() {
        let conn = test_connection();
        insert(&conn, &sample_record("id-oldest", 1_000)).unwrap();
        insert(&conn, &sample_record("id-middle", 2_000)).unwrap();
        insert(&conn, &sample_record("id-newest", 3_000)).unwrap();

        let all = list_all(&conn).expect("list_all succeeds");
        let ids: Vec<&str> = all.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["id-newest", "id-middle", "id-oldest"]);
    }

    #[test]
    fn list_all_returns_empty_when_no_rows() {
        let conn = test_connection();
        let all = list_all(&conn).expect("list_all on empty db");
        assert!(all.is_empty());
    }

    #[test]
    fn delete_existing_returns_one() {
        let conn = test_connection();
        let doc = sample_record("to-delete", 1);
        insert(&conn, &doc).unwrap();

        let affected = delete(&conn, &doc.id).expect("delete succeeds");
        assert_eq!(affected, 1);

        let fetched = get(&conn, &doc.id).expect("get after delete");
        assert!(fetched.is_none(), "row should be gone after delete");
    }

    #[test]
    fn delete_nonexistent_returns_zero() {
        let conn = test_connection();
        let affected = delete(&conn, "never-existed").expect("delete is Ok");
        assert_eq!(affected, 0);
    }

    #[test]
    fn optional_fields_persist_as_none() {
        let conn = test_connection();
        let doc = DocumentRecord {
            id: "err-row".to_string(),
            title: "Failed synth".to_string(),
            source_type: "paste".to_string(),
            char_count: 0,
            voice: "Nec_24000".to_string(),
            status: "error".to_string(),
            error_message: Some("HTTP 500 from Sberbank".to_string()),
            created_at: 1_700_000_000_000,
            audio_path: None,
            audio_duration_ms: None,
        };

        insert(&conn, &doc).expect("insert with NULLs succeeds");
        let fetched = get(&conn, &doc.id).expect("get").expect("row present");

        assert_eq!(fetched.audio_path, None);
        assert_eq!(fetched.audio_duration_ms, None);
        assert_eq!(
            fetched.error_message,
            Some("HTTP 500 from Sberbank".to_string())
        );
        assert_eq!(fetched.status, "error");
    }

    #[test]
    fn update_title_returns_one_row_affected_and_persists_change() {
        let conn = test_connection();
        let original = sample_record("doc-rename", 1_700_000_000_000);
        insert(&conn, &original).unwrap();

        let rows = update_title(&conn, &original.id, "Новое имя файла").unwrap();
        assert_eq!(rows, 1);

        let fetched = get(&conn, &original.id).unwrap().unwrap();
        assert_eq!(fetched.title, "Новое имя файла");
        // Other fields are untouched.
        assert_eq!(fetched.created_at, original.created_at);
        assert_eq!(fetched.audio_path, original.audio_path);
        assert_eq!(fetched.voice, original.voice);
    }

    #[test]
    fn update_title_returns_zero_rows_affected_for_unknown_id() {
        let conn = test_connection();
        let rows = update_title(&conn, "does-not-exist", "any title").unwrap();
        assert_eq!(rows, 0);
    }
}
