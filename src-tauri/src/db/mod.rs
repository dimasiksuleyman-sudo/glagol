//! Database module: SQLite connection management + migrations + repository.
//!
//! The application uses a single embedded SQLite database
//! (`%LOCALAPPDATA%\Glagol\glagol.db` on Windows) opened eagerly at startup.
//! Migrations are applied through [`rusqlite_migration`] and the resulting
//! [`Connection`] is stored on [`crate::state::AppState`] behind a
//! `std::sync::Mutex` — rusqlite is synchronous, so an async lock would only
//! add ceremony.

pub mod migrations;
pub mod repository;

use std::fs;
use std::path::Path;

use rusqlite::Connection;

/// Open the application database, apply pending migrations, and return the
/// ready-to-use [`Connection`].
///
/// The parent directory of `db_path` is created if it does not yet exist
/// (first launch). Any failure — directory creation, opening the file, or
/// running the migrations — is reported as a human-readable error so the
/// Tauri setup hook can surface it cleanly.
pub fn init_database(db_path: &Path) -> Result<Connection, String> {
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create database directory: {e}"))?;
    }
    let mut conn =
        Connection::open(db_path).map_err(|e| format!("Failed to open database: {e}"))?;
    migrations::apply_migrations(&mut conn)
        .map_err(|e| format!("Database migration failed: {e}"))?;
    Ok(conn)
}

#[cfg(test)]
pub(crate) fn test_connection() -> Connection {
    let mut conn = Connection::open_in_memory().expect("open in-memory connection");
    migrations::apply_migrations(&mut conn).expect("apply migrations on in-memory db");
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_database_creates_file_and_applies_schema() {
        let dir = std::env::temp_dir().join(format!("glagol_init_test_{}", uuid::Uuid::new_v4()));
        let db_path = dir.join("nested").join("glagol.db");

        let conn = init_database(&db_path).expect("init_database succeeds");

        assert!(db_path.exists(), "database file should be created on disk");

        let table: String = conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='documents'",
                [],
                |row| row.get(0),
            )
            .expect("documents table should exist after init");
        assert_eq!(table, "documents");

        drop(conn);
        let _ = fs::remove_dir_all(&dir);
    }
}
