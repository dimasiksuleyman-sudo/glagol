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

// ── api_usage table ────────────────────────────────────────────────────
//
// Sprint 5d. The `api_usage` table tracks per-month SaluteSpeech
// consumption so the Settings page can show "X / 200 000 chars used
// this month". One row per `YYYY-MM` calendar month (local timezone).
// `recognitions_seconds` is reserved for a future STT feature; Sprint
// 5d only writes `chars_used`.

/// Add `chars_added` to the running `chars_used` total for `month`,
/// inserting a fresh row at zero if this is the first synthesis of the
/// month. `month` is expected to be in `YYYY-MM` form; the function
/// does not validate the shape (the caller — `commands::usage` — owns
/// that). `updated_at` is the current Unix millisecond timestamp.
///
/// Advisory write: the synthesis pipeline calls this *after* a
/// successful audio write, so a failure here means the counter is
/// merely stale, not that the document is missing. Callers should log
/// rather than surface a user-facing error.
pub fn record_usage(
    conn: &Connection,
    month: &str,
    chars_added: i64,
    updated_at: i64,
) -> Result<usize> {
    conn.execute(
        "
        INSERT INTO api_usage (month, chars_used, recognitions_seconds, updated_at)
        VALUES (?1, ?2, 0, ?3)
        ON CONFLICT(month) DO UPDATE SET
            chars_used = chars_used + excluded.chars_used,
            updated_at = excluded.updated_at
        ",
        params![month, chars_added, updated_at],
    )
}

/// Snapshot of a single `api_usage` row. Returned by
/// [`get_usage_for_month`] as `None` when the month has never been
/// written — the caller is expected to render that as a zero-state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRow {
    pub month: String,
    pub chars_used: i64,
    pub recognitions_seconds: i64,
    pub updated_at: i64,
}

/// Look up the usage row for a single month. Returns `Ok(None)` when
/// the month has not been recorded yet — the Settings page treats that
/// case as `chars_used = 0`.
pub fn get_usage_for_month(conn: &Connection, month: &str) -> Result<Option<UsageRow>> {
    conn.query_row(
        "SELECT month, chars_used, recognitions_seconds, updated_at \
         FROM api_usage WHERE month = ?1",
        params![month],
        |row| {
            Ok(UsageRow {
                month: row.get(0)?,
                chars_used: row.get(1)?,
                recognitions_seconds: row.get(2)?,
                updated_at: row.get(3)?,
            })
        },
    )
    .optional()
}

/// Add `seconds_added` to the running `recognitions_seconds` total for
/// `month`, inserting a fresh row (with `chars_used = 0`) if this is the first
/// activity of the month. Mirrors [`record_usage`] but writes the STT column
/// reserved back in Sprint 5d.
///
/// `month` is `YYYY-MM`; `updated_at` is a Unix millisecond timestamp. Both
/// are supplied by the command layer so this function stays deterministic
/// (no clock access). Advisory write — the dictation pipeline calls it after a
/// successful transcription and logs rather than surfaces a failure.
pub fn record_recognition_usage(
    conn: &Connection,
    month: &str,
    seconds_added: i64,
    updated_at: i64,
) -> Result<usize> {
    conn.execute(
        "
        INSERT INTO api_usage (month, chars_used, recognitions_seconds, updated_at)
        VALUES (?1, 0, ?2, ?3)
        ON CONFLICT(month) DO UPDATE SET
            recognitions_seconds = recognitions_seconds + excluded.recognitions_seconds,
            updated_at = excluded.updated_at
        ",
        params![month, seconds_added, updated_at],
    )
}

// ── app_settings table ─────────────────────────────────────────────────
//
// Sprint 6 PR1 (Dictation). Generic key-value store for non-secret
// configuration. The STT feature persists `stt_base_url`, `stt_model`,
// `stt_proxy` and `stt_language` here; the API key stays in the OS keyring.

// The parameter is named `setting_name` (not `key`) throughout the
// app_settings API: a future secret-redaction scan keys off the literal
// `key` and would false-positive on `get_setting(key)` (kickoff D3).

/// Upsert a single setting. `updated_at` is a Unix millisecond timestamp
/// supplied by the caller (deterministic — no clock access here).
pub fn set_setting(
    conn: &Connection,
    setting_name: &str,
    value: &str,
    updated_at: i64,
) -> Result<usize> {
    conn.execute(
        "
        INSERT INTO app_settings (key, value, updated_at)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            updated_at = excluded.updated_at
        ",
        params![setting_name, value, updated_at],
    )
}

/// Read a single setting's value. Returns `Ok(None)` when the key has never
/// been written — the caller substitutes its default.
pub fn get_setting(conn: &Connection, setting_name: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![setting_name],
        |row| row.get(0),
    )
    .optional()
}

// ── dictations table ───────────────────────────────────────────────────
//
// Sprint 6 PR5a (Dictation history). One row per completed dictation,
// written only when `dictation_history_enabled` is on (gating lives at the
// session layer, D4). `status` mirrors the pipeline's `Disposition` (D2):
// 'pasted' | 'clipboard' | 'error'.

/// Maximum number of history rows kept in `dictations`. The table is pruned to
/// this cap **inside the same transaction as every insert** (D5), so it can
/// never exceed the cap even momentarily.
pub const DICTATION_HISTORY_CAP: i64 = 200;

/// A dictation about to be persisted. No `id` — the column is
/// `INTEGER PRIMARY KEY AUTOINCREMENT`, assigned by SQLite. `created_at` /
/// `duration_ms` are Unix-ms / milliseconds supplied by the caller so this
/// layer stays clock-free and deterministic (matching [`DocumentRecord`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDictation {
    pub created_at: i64,
    pub duration_ms: i64,
    pub text: String,
    /// 'pasted' | 'clipboard' | 'error' (D2).
    pub status: String,
    pub error_message: Option<String>,
}

/// A persisted dictation row, including its assigned `id`. Returned by
/// [`list_dictations`]. `Serialize` so the command layer can hand it straight
/// to the (PR5b) Settings history list over IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Dictation {
    pub id: i64,
    pub created_at: i64,
    pub duration_ms: i64,
    pub text: String,
    pub status: String,
    pub error_message: Option<String>,
}

impl Dictation {
    fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            created_at: row.get("created_at")?,
            duration_ms: row.get("duration_ms")?,
            text: row.get("text")?,
            status: row.get("status")?,
            error_message: row.get("error_message")?,
        })
    }
}

/// Insert a dictation and prune the table back to [`DICTATION_HISTORY_CAP`] in
/// **one transaction** (D5), so the history can never exceed the cap. Returns
/// the new row's `id`. The prune orders by `created_at DESC, id DESC` — `id`
/// breaks ties deterministically (autoincrement is monotonic with insertion),
/// so the oldest rows are always the ones dropped even when several share a
/// timestamp. Takes `&mut Connection` because it opens a transaction.
pub fn insert_dictation(conn: &mut Connection, rec: &NewDictation) -> Result<i64> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO dictations (created_at, duration_ms, text, status, error_message) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            rec.created_at,
            rec.duration_ms,
            rec.text,
            rec.status,
            rec.error_message
        ],
    )?;
    let id = tx.last_insert_rowid();
    tx.execute(
        "DELETE FROM dictations WHERE id NOT IN (
             SELECT id FROM dictations ORDER BY created_at DESC, id DESC LIMIT ?1
         )",
        params![DICTATION_HISTORY_CAP],
    )?;
    tx.commit()?;
    Ok(id)
}

/// List dictations, most recent first (`created_at DESC, id DESC`). `limit`
/// caps the number of rows returned (`None` = all, up to the table's own
/// [`DICTATION_HISTORY_CAP`]). Whether to return anything at all when history is
/// disabled is a policy decision owned by the command layer, not here.
pub fn list_dictations(conn: &Connection, limit: Option<i64>) -> Result<Vec<Dictation>> {
    let sql = format!(
        "SELECT id, created_at, duration_ms, text, status, error_message \
         FROM dictations ORDER BY created_at DESC, id DESC{}",
        match limit {
            Some(_) => " LIMIT ?1",
            None => "",
        }
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = match limit {
        Some(n) => stmt.query_map(params![n], Dictation::from_row)?.collect(),
        None => stmt.query_map([], Dictation::from_row)?.collect(),
    };
    rows
}

/// Delete every dictation. Returns the number of rows removed. Backs the
/// Settings «Очистить историю» button (D5).
pub fn clear_dictations(conn: &Connection) -> Result<usize> {
    conn.execute("DELETE FROM dictations", [])
}

/// Total recognition seconds across every recorded month — the lifetime
/// «Надиктовано» figure (D4). Reads the always-on `api_usage.recognitions_seconds`
/// column (independent of the history toggle), summing all months so the stat is
/// a lifetime total rather than resetting each month like the TTS free-tier
/// counter. `SUM` over an empty table returns `NULL`, coalesced to `0`.
pub fn sum_recognition_seconds(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT COALESCE(SUM(recognitions_seconds), 0) FROM api_usage",
        [],
        |row| row.get(0),
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

    // ── api_usage table tests ─────────────────────────────────────

    #[test]
    fn record_usage_inserts_new_month_row() {
        let conn = test_connection();
        let now = 1_700_000_000_000;
        let affected = record_usage(&conn, "2026-05", 100, now).expect("insert");
        assert_eq!(affected, 1);

        let row = get_usage_for_month(&conn, "2026-05")
            .expect("select")
            .expect("row exists");
        assert_eq!(row.month, "2026-05");
        assert_eq!(row.chars_used, 100);
        assert_eq!(row.recognitions_seconds, 0);
        assert_eq!(row.updated_at, now);
    }

    #[test]
    fn record_usage_increments_existing_month_row() {
        let conn = test_connection();
        let first = 1_700_000_000_000;
        let second = 1_700_000_010_000;

        record_usage(&conn, "2026-05", 50, first).expect("first write");
        record_usage(&conn, "2026-05", 75, second).expect("second write");

        let row = get_usage_for_month(&conn, "2026-05")
            .expect("select")
            .expect("row exists");
        assert_eq!(row.chars_used, 125, "second call must add, not replace");
        assert_eq!(
            row.updated_at, second,
            "updated_at must reflect the most recent write"
        );
        assert_eq!(row.recognitions_seconds, 0);
    }

    #[test]
    fn get_usage_for_month_returns_none_for_missing_month() {
        let conn = test_connection();
        let row = get_usage_for_month(&conn, "2026-05").expect("query ok");
        assert!(row.is_none(), "missing month must surface as None");
    }

    #[test]
    fn record_usage_isolates_months() {
        // Two distinct months track independently — incrementing May
        // does not affect June, and vice versa. Guards the natural
        // calendar-boundary rollover semantics promised by the
        // Settings counter.
        let conn = test_connection();
        record_usage(&conn, "2026-05", 1_000, 1_700_000_000_000).unwrap();
        record_usage(&conn, "2026-06", 250, 1_700_000_001_000).unwrap();
        record_usage(&conn, "2026-05", 500, 1_700_000_002_000).unwrap();

        let may = get_usage_for_month(&conn, "2026-05").unwrap().unwrap();
        let june = get_usage_for_month(&conn, "2026-06").unwrap().unwrap();
        assert_eq!(may.chars_used, 1_500);
        assert_eq!(june.chars_used, 250);
    }

    #[test]
    fn record_recognition_usage_inserts_and_increments() {
        let conn = test_connection();
        let first = 1_700_000_000_000;
        let second = 1_700_000_005_000;

        let affected = record_recognition_usage(&conn, "2026-07", 12, first).expect("insert");
        assert_eq!(affected, 1);

        let row = get_usage_for_month(&conn, "2026-07").unwrap().unwrap();
        assert_eq!(row.recognitions_seconds, 12);
        assert_eq!(row.chars_used, 0, "STT write must not touch chars_used");
        assert_eq!(row.updated_at, first);

        record_recognition_usage(&conn, "2026-07", 8, second).expect("increment");
        let row = get_usage_for_month(&conn, "2026-07").unwrap().unwrap();
        assert_eq!(
            row.recognitions_seconds, 20,
            "second call must add, not replace"
        );
        assert_eq!(row.chars_used, 0);
        assert_eq!(row.updated_at, second);
    }

    #[test]
    fn record_recognition_and_chars_usage_are_independent_columns() {
        // A TTS char write and an STT seconds write to the same month must
        // each land in their own column without clobbering the other.
        let conn = test_connection();
        record_usage(&conn, "2026-07", 500, 1_700_000_000_000).unwrap();
        record_recognition_usage(&conn, "2026-07", 30, 1_700_000_001_000).unwrap();

        let row = get_usage_for_month(&conn, "2026-07").unwrap().unwrap();
        assert_eq!(row.chars_used, 500);
        assert_eq!(row.recognitions_seconds, 30);
    }

    // ── app_settings tests ────────────────────────────────────────

    #[test]
    fn set_setting_then_get_returns_value() {
        let conn = test_connection();
        let affected = set_setting(
            &conn,
            "stt_model",
            "whisper-large-v3-turbo",
            1_700_000_000_000,
        )
        .unwrap();
        assert_eq!(affected, 1);
        assert_eq!(
            get_setting(&conn, "stt_model").unwrap(),
            Some("whisper-large-v3-turbo".to_string())
        );
    }

    #[test]
    fn get_setting_returns_none_for_unset_key() {
        let conn = test_connection();
        assert_eq!(get_setting(&conn, "stt_base_url").unwrap(), None);
    }

    #[test]
    fn set_setting_upserts_on_conflict() {
        let conn = test_connection();
        set_setting(&conn, "stt_language", "ru", 1_700_000_000_000).unwrap();
        set_setting(&conn, "stt_language", "en", 1_700_000_010_000).unwrap();

        assert_eq!(
            get_setting(&conn, "stt_language").unwrap(),
            Some("en".to_string()),
            "second write must replace the value"
        );
    }

    // ── dictations tests ──────────────────────────────────────────

    fn sample_dictation(created_at: i64, text: &str) -> NewDictation {
        NewDictation {
            created_at,
            duration_ms: 1_500,
            text: text.to_string(),
            status: "clipboard".to_string(),
            error_message: None,
        }
    }

    fn dictation_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM dictations", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn insert_dictation_then_list_returns_it() {
        let mut conn = test_connection();
        let id = insert_dictation(&mut conn, &sample_dictation(1_000, "привет мир")).unwrap();
        assert!(id > 0, "autoincrement id should be positive");

        let all = list_dictations(&conn, None).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, id);
        assert_eq!(all[0].text, "привет мир");
        assert_eq!(all[0].status, "clipboard");
        assert_eq!(all[0].duration_ms, 1_500);
        assert_eq!(all[0].error_message, None);
    }

    #[test]
    fn insert_dictation_persists_error_status_and_message() {
        let mut conn = test_connection();
        insert_dictation(
            &mut conn,
            &NewDictation {
                created_at: 1_000,
                duration_ms: 0,
                text: String::new(),
                status: "error".to_string(),
                error_message: Some("провайдер отклонил запрос".to_string()),
            },
        )
        .unwrap();

        let all = list_dictations(&conn, None).unwrap();
        assert_eq!(all[0].status, "error");
        assert_eq!(
            all[0].error_message,
            Some("провайдер отклонил запрос".to_string())
        );
        assert_eq!(all[0].text, "");
    }

    #[test]
    fn list_dictations_orders_newest_first() {
        let mut conn = test_connection();
        insert_dictation(&mut conn, &sample_dictation(1_000, "старое")).unwrap();
        insert_dictation(&mut conn, &sample_dictation(3_000, "новое")).unwrap();
        insert_dictation(&mut conn, &sample_dictation(2_000, "среднее")).unwrap();

        let texts: Vec<String> = list_dictations(&conn, None)
            .unwrap()
            .into_iter()
            .map(|d| d.text)
            .collect();
        assert_eq!(texts, vec!["новое", "среднее", "старое"]);
    }

    #[test]
    fn list_dictations_respects_limit() {
        let mut conn = test_connection();
        for i in 0..5 {
            insert_dictation(&mut conn, &sample_dictation(1_000 + i, "x")).unwrap();
        }
        assert_eq!(list_dictations(&conn, Some(2)).unwrap().len(), 2);
        assert_eq!(list_dictations(&conn, None).unwrap().len(), 5);
    }

    #[test]
    fn list_dictations_empty_when_no_rows() {
        let conn = test_connection();
        assert!(list_dictations(&conn, None).unwrap().is_empty());
    }

    #[test]
    fn clear_dictations_removes_all() {
        let mut conn = test_connection();
        insert_dictation(&mut conn, &sample_dictation(1_000, "a")).unwrap();
        insert_dictation(&mut conn, &sample_dictation(2_000, "b")).unwrap();
        let removed = clear_dictations(&conn).unwrap();
        assert_eq!(removed, 2);
        assert_eq!(dictation_count(&conn), 0);
    }

    #[test]
    fn insert_dictation_prunes_to_cap_dropping_oldest() {
        // D5 guard: inserting past the cap must leave EXACTLY `DICTATION_HISTORY_CAP`
        // rows, with the oldest gone. This is the negative-cycle test — deleting
        // the prune DELETE from `insert_dictation` makes the count assertion fail.
        let mut conn = test_connection();
        let cap = DICTATION_HISTORY_CAP;

        // Insert cap + 1 rows with strictly increasing timestamps; the very first
        // (oldest) must be the one pruned.
        let oldest_id = insert_dictation(&mut conn, &sample_dictation(1_000, "oldest")).unwrap();
        for i in 1..=cap {
            insert_dictation(&mut conn, &sample_dictation(1_000 + i, "x")).unwrap();
        }

        assert_eq!(
            dictation_count(&conn),
            cap,
            "table must be pruned to exactly the cap"
        );
        let survivors = list_dictations(&conn, None).unwrap();
        assert!(
            survivors.iter().all(|d| d.id != oldest_id),
            "the oldest row must have been pruned"
        );
        assert!(
            survivors.iter().all(|d| d.text != "oldest"),
            "the oldest transcript must be gone"
        );
    }

    #[test]
    fn sum_recognition_seconds_totals_across_months() {
        let conn = test_connection();
        assert_eq!(
            sum_recognition_seconds(&conn).unwrap(),
            0,
            "empty ledger sums to 0, not NULL"
        );
        record_recognition_usage(&conn, "2026-05", 30, 1).unwrap();
        record_recognition_usage(&conn, "2026-06", 45, 2).unwrap();
        record_usage(&conn, "2026-06", 999, 3).unwrap(); // chars must not count
        assert_eq!(sum_recognition_seconds(&conn).unwrap(), 75);
    }
}
