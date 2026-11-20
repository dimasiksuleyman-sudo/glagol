//! The pronunciation dictionary (tts-normalization N2): storage in SQLite and
//! the commands of Settings → Text to speech → Pronunciation dictionary.

use rusqlite::{params, Connection, OptionalExtension};
use tauri::State;

use crate::state::AppState;
use crate::text::dictionary::{self, Entry, MAX_ENTRIES};

fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    Ok(Entry {
        id: Some(row.get(0)?),
        pattern: row.get(1)?,
        replacement: row.get(2)?,
        language: row.get(3)?,
        whole_word: row.get::<_, i64>(4)? != 0,
        case_sensitive: row.get::<_, i64>(5)? != 0,
        enabled: row.get::<_, i64>(6)? != 0,
    })
}

/// Every entry, alphabetically.
pub fn list(conn: &Connection) -> rusqlite::Result<Vec<Entry>> {
    let mut stmt = conn.prepare(
        "SELECT id, pattern, replacement, language, whole_word, case_sensitive, enabled
         FROM pronunciations ORDER BY pattern COLLATE NOCASE, language",
    )?;
    let rows = stmt.query_map([], row_to_entry)?;
    rows.collect()
}

/// Insert or update an entry (by id, or by pattern + language when new).
pub fn save(conn: &Connection, entry: Entry, now: i64) -> Result<Entry, String> {
    let entry = dictionary::validate(entry)?;
    let existing: Option<i64> = match entry.id {
        Some(id) => Some(id),
        None => conn
            .query_row(
                "SELECT id FROM pronunciations WHERE pattern = ?1 AND language = ?2",
                params![entry.pattern, entry.language],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?,
    };
    let id = match existing {
        Some(id) => {
            conn.execute(
                "UPDATE pronunciations SET pattern = ?1, replacement = ?2, language = ?3,
                 whole_word = ?4, case_sensitive = ?5, enabled = ?6, updated_at = ?7 WHERE id = ?8",
                params![
                    entry.pattern,
                    entry.replacement,
                    entry.language,
                    entry.whole_word,
                    entry.case_sensitive,
                    entry.enabled,
                    now,
                    id
                ],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(_, _) => {
                    "Такое слово для этого языка уже есть в словаре.".to_string()
                }
                other => other.to_string(),
            })?;
            id
        }
        None => {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM pronunciations", [], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            if count as usize >= MAX_ENTRIES {
                return Err("В словаре уже максимальное число записей.".into());
            }
            conn.execute(
                "INSERT INTO pronunciations
                 (pattern, replacement, language, whole_word, case_sensitive, enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![
                    entry.pattern,
                    entry.replacement,
                    entry.language,
                    entry.whole_word,
                    entry.case_sensitive,
                    entry.enabled,
                    now
                ],
            )
            .map_err(|e| e.to_string())?;
            conn.last_insert_rowid()
        }
    };
    Ok(Entry {
        id: Some(id),
        ..entry
    })
}

/// Delete one entry.
pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM pronunciations WHERE id = ?1", params![id])
}

/// Entries for synthesis in `language`; an unreadable dictionary counts as
/// empty so synthesis never fails because of it.
pub fn for_synthesis(conn: &Connection) -> Vec<Entry> {
    list(conn).unwrap_or_else(|e| {
        tracing::warn!("pronunciation dictionary unreadable: {e}");
        Vec::new()
    })
}

/// Import CSV text: new patterns are added, existing ones updated. Returns
/// (imported, skipped).
pub fn import(conn: &mut Connection, csv: &str, now: i64) -> Result<(usize, usize), String> {
    let (entries, mut skipped) = dictionary::from_csv(csv);
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut imported = 0;
    for entry in entries {
        match save(&tx, entry, now) {
            Ok(_) => imported += 1,
            Err(_) => skipped += 1,
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok((imported, skipped))
}

fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[tauri::command]
pub fn list_pronunciations(state: State<'_, AppState>) -> Result<Vec<Entry>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    list(&conn).map_err(|e| crate::i18n::error(format!("Не удалось прочитать словарь: {e}")))
}

#[tauri::command]
pub fn save_pronunciation(state: State<'_, AppState>, entry: Entry) -> Result<Entry, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    save(&conn, entry, now()).map_err(crate::i18n::error)
}

#[tauri::command]
pub fn delete_pronunciation(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    delete(&conn, id)
        .map(|_| ())
        .map_err(|e| crate::i18n::error(format!("Не удалось удалить запись словаря: {e}")))
}

/// Write the dictionary as CSV to a path the user chose in a save dialog.
#[tauri::command]
pub fn export_pronunciations(state: State<'_, AppState>, path: String) -> Result<usize, String> {
    let entries = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        list(&conn).map_err(|e| e.to_string())?
    };
    std::fs::write(&path, dictionary::to_csv(&entries))
        .map_err(|e| crate::i18n::error(format!("Не удалось сохранить словарь: {e}")))?;
    Ok(entries.len())
}

/// Read a CSV file the user chose in an open dialog (at most 1 MB).
#[tauri::command]
pub fn import_pronunciations(
    state: State<'_, AppState>,
    path: String,
) -> Result<(usize, usize), String> {
    let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    if size > 1024 * 1024 {
        return Err(crate::i18n::error("Файл словаря больше 1 МБ.".to_string()));
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| crate::i18n::error(format!("Не удалось прочитать файл словаря: {e}")))?;
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    import(&mut conn, &text, now()).map_err(crate::i18n::error)
}

/// How a phrase will be read: the dictionary, then the built-in rules.
#[tauri::command]
pub fn preview_normalized(
    state: State<'_, AppState>,
    text: String,
    language: String,
) -> Result<String, String> {
    let text: String = text.chars().take(2000).collect();
    let entries = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        for_synthesis(&conn)
    };
    Ok(prepare_for_speech(&text, &entries, &language))
}

/// The full text preparation for speech: dictionary first, then the
/// built-in preprocessing and normalization. Russian keeps «+» stress marks
/// (Silero honours them); English drops them.
pub fn prepare_for_speech(text: &str, entries: &[Entry], language: &str) -> String {
    let text = dictionary::apply(text, entries, if language == "en" { "en" } else { "ru" });
    crate::text::preprocessor::after_dictionary(&text, language)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        crate::db::test_connection()
    }

    fn entry(pattern: &str, replacement: &str) -> Entry {
        Entry {
            id: None,
            pattern: pattern.into(),
            replacement: replacement.into(),
            language: "any".into(),
            whole_word: true,
            case_sensitive: false,
            enabled: true,
        }
    }

    #[test]
    fn save_update_list_and_delete() {
        let conn = db();
        let saved = save(&conn, entry("IPO", "ай-пи-о"), 1).unwrap();
        assert!(saved.id.is_some());
        // Same pattern + language updates instead of duplicating.
        let again = save(&conn, entry("IPO", "ипо"), 2).unwrap();
        assert_eq!(again.id, saved.id);
        save(&conn, entry("ВВП", "вэ-вэ-пэ"), 3).unwrap();
        let all = list(&conn).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(
            all.iter().find(|e| e.pattern == "IPO").unwrap().replacement,
            "ипо"
        );
        delete(&conn, saved.id.unwrap()).unwrap();
        assert_eq!(list(&conn).unwrap().len(), 1);
        assert!(save(&conn, entry("", "x"), 4).is_err());
    }

    #[test]
    fn import_adds_and_updates() {
        let mut conn = db();
        save(&conn, entry("ЦБ", "центробанк"), 1).unwrap();
        let (imported, skipped) = import(&mut conn, "ЦБ;цэ-бэ\nМВФ;эм-вэ-эф\n;пусто\n", 2).unwrap();
        assert_eq!((imported, skipped), (2, 1));
        let all = list(&conn).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(
            all.iter().find(|e| e.pattern == "ЦБ").unwrap().replacement,
            "цэ-бэ"
        );
    }

    #[test]
    fn dictionary_wins_over_built_in_rules() {
        let entries = vec![
            entry("IPO", "первичное размещение"),
            entry("Anthropic", "Энтропик"),
        ];
        assert_eq!(
            prepare_for_speech("IPO Anthropic на 5%", &entries, "ru"),
            "первичное размещение Энтропик на пять процентов"
        );
    }

    #[test]
    fn stress_marks_kept_for_russian_dropped_for_english() {
        let mut lock = entry("замок", "з+амок");
        lock.language = "ru".into();
        let mut name = entry("Glagol", "Gl+agol");
        name.language = "en".into();
        let entries = vec![lock, name];
        assert_eq!(prepare_for_speech("замок", &entries, "ru"), "з+амок");
        assert_eq!(prepare_for_speech("Glagol", &entries, "en"), "Glagol");
    }
}
