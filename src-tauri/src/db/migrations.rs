//! Schema migrations using `rusqlite_migration`.
//!
//! Each [`M::up`] is applied atomically (the crate wraps it in a transaction)
//! and the schema version is tracked via SQLite's `user_version` PRAGMA.
//! Adding a new migration means appending to [`MIGRATIONS_SLICE`] — never
//! edit an existing entry once it has shipped to users.

use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

const MIGRATIONS_SLICE: &[M<'static>] = &[
    M::up(
        r#"
    CREATE TABLE documents (
        id                  TEXT PRIMARY KEY NOT NULL,
        title               TEXT NOT NULL,
        source_type         TEXT NOT NULL,
        char_count          INTEGER NOT NULL,
        voice               TEXT NOT NULL,
        status              TEXT NOT NULL,
        error_message       TEXT,
        created_at          INTEGER NOT NULL,
        audio_path          TEXT,
        audio_duration_ms   INTEGER
    );
    CREATE INDEX idx_docs_created ON documents(created_at DESC);
    "#,
    ),
    // Sprint 5d: per-month SaluteSpeech usage ledger for the free-tier
    // counter on the Settings page. One row per calendar month
    // (`'YYYY-MM'`, local timezone). `recognitions_seconds` is reserved
    // for a future STT feature; Sprint 5d only writes `chars_used`.
    M::up(
        r#"
    CREATE TABLE api_usage (
        month                   TEXT PRIMARY KEY NOT NULL,
        chars_used              INTEGER NOT NULL DEFAULT 0,
        recognitions_seconds    INTEGER NOT NULL DEFAULT 0,
        updated_at              INTEGER NOT NULL
    );
    "#,
    ),
    // Sprint 6 PR1 (Dictation): generic key-value settings store. Holds the
    // non-secret STT configuration (`stt_base_url`, `stt_model`, `stt_proxy`,
    // `stt_language`); the STT API key itself lives in the OS keyring, never
    // here. One row per setting key. Append-only — never edit a shipped
    // migration.
    M::up(
        r#"
    CREATE TABLE app_settings (
        key         TEXT PRIMARY KEY NOT NULL,
        value       TEXT NOT NULL,
        updated_at  INTEGER NOT NULL
    );
    "#,
    ),
];

/// Apply every pending migration to `conn`. Idempotent: calling twice on the
/// same database is a no-op the second time.
pub fn apply_migrations(conn: &mut Connection) -> Result<(), rusqlite_migration::Error> {
    let migrations = Migrations::from_slice(MIGRATIONS_SLICE);
    migrations.validate()?;
    migrations.to_latest(conn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn migrations_validate_successfully() {
        let migrations = Migrations::from_slice(MIGRATIONS_SLICE);
        migrations
            .validate()
            .expect("MIGRATIONS_SLICE is well-formed");
    }

    #[test]
    fn apply_migrations_to_empty_db_creates_documents_table() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply_migrations(&mut conn).expect("apply succeeds");

        let n = count(
            &conn,
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='documents'",
        );
        assert_eq!(n, 1, "documents table should be created exactly once");
    }

    #[test]
    fn apply_migrations_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply_migrations(&mut conn).expect("first apply succeeds");
        apply_migrations(&mut conn).expect("second apply is a no-op");

        let n = count(
            &conn,
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='documents'",
        );
        assert_eq!(n, 1, "documents table should still exist exactly once");
    }

    #[test]
    fn apply_migrations_creates_app_settings_table() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply_migrations(&mut conn).expect("apply succeeds");

        let n = count(
            &conn,
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='app_settings'",
        );
        assert_eq!(n, 1, "app_settings table should be created by migration v3");
    }

    #[test]
    fn apply_migrations_creates_index() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply_migrations(&mut conn).expect("apply succeeds");

        let n = count(
            &conn,
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_docs_created'",
        );
        assert_eq!(n, 1, "idx_docs_created index should exist after migration");
    }
}
