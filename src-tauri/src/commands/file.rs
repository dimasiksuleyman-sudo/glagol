//! `read_and_parse_file` Tauri command and its testable inner.
//!
//! The command is the single entry point the frontend uses after the
//! user picks a file via `dialog.open()`. It enforces two limits up
//! front and dispatches to the appropriate parser by file extension,
//! falling back to [`parser::try_all`] for the "Все файлы" escape hatch.
//!
//! Limits (Sprint 4 Q6 — conservative defaults):
//!
//! * **10 MB file size** — measured via `fs::metadata` before any
//!   parsing so a 200 MB DOCX is rejected without spending time
//!   unzipping it.
//! * **500 000 characters** of extracted text — measured post-parse
//!   via `chars().count()` so Cyrillic is counted correctly. Anchored
//!   to the SaluteSpeech monthly quota (200 000 chars × ~2.5
//!   documents) — both numbers can be relaxed in Sprint 5+ once we
//!   have real-world usage data.

use std::fs;
use std::path::Path;

use crate::parser::{self, ParseError, ParsedDocument};

/// 10 MB hard limit on raw file size. Anything larger is rejected
/// before the parser is invoked at all.
pub(crate) const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// 500 000 characters of extracted text. `chars().count()` so Cyrillic
/// is one char per letter, not per UTF-8 byte.
pub(crate) const MAX_CONTENT_CHARS: usize = 500_000;

#[tauri::command]
pub async fn read_and_parse_file(path: String) -> Result<ParsedDocument, String> {
    read_and_parse_file_impl(Path::new(&path)).map_err(|e| e.to_string())
}

pub(crate) fn read_and_parse_file_impl(path: &Path) -> Result<ParsedDocument, ParseError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_FILE_SIZE {
        let mb = metadata.len() as f64 / (1024.0 * 1024.0);
        return Err(ParseError::Format(format!(
            "Файл слишком большой ({mb:.1} MB из 10 MB лимита)"
        )));
    }

    let doc = dispatch_by_extension(path)?;

    let char_count = doc.text.chars().count();
    if char_count > MAX_CONTENT_CHARS {
        let k = char_count as f64 / 1000.0;
        return Err(ParseError::Format(format!(
            "Документ слишком длинный ({k:.0}K символов из 500K лимита)"
        )));
    }

    Ok(doc)
}

fn dispatch_by_extension(path: &Path) -> Result<ParsedDocument, ParseError> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    match extension.as_deref() {
        Some("txt") => parser::txt::parse(path),
        Some("md") | Some("markdown") => parser::md::parse(path),
        Some("docx") => parser::docx::parse(path),
        Some("pdf") => parser::pdf::parse(path),
        // Unknown / missing extension → "Все файлы" escape hatch.
        _ => parser::try_all(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn unique_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "glagol_cmd_file_{}_{}",
            label,
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn dispatches_txt_by_extension() {
        let dir = unique_dir("txt_dispatch");
        let path = dir.join("note.txt");
        fs::write(&path, "Простой текст.").unwrap();

        let doc = read_and_parse_file_impl(&path).expect("parse ok");
        assert_eq!(doc.source_format, "txt");
        assert!(doc.text.contains("Простой текст."));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dispatches_md_by_extension() {
        let dir = unique_dir("md_dispatch");
        let path = dir.join("note.md");
        fs::write(&path, "# Заголовок\n\nТекст.").unwrap();

        let doc = read_and_parse_file_impl(&path).expect("parse ok");
        assert_eq!(doc.source_format, "md");
        assert!(doc.text.contains("Заголовок"));
        assert!(doc.text.contains("Текст."));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_files_above_size_limit() {
        let dir = unique_dir("too_big");
        let path = dir.join("huge.txt");
        // Just over the 10 MB cap. Built with one `write_all` of zeros
        // to keep the test fast; the parser is never reached.
        let bytes = vec![b' '; (MAX_FILE_SIZE + 1) as usize];
        fs::write(&path, &bytes).unwrap();

        let err = read_and_parse_file_impl(&path).unwrap_err();
        match err {
            ParseError::Format(msg) => assert!(
                msg.contains("слишком большой"),
                "expected size-limit error, got: {msg}"
            ),
            other => panic!("expected ParseError::Format, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn falls_through_to_try_all_for_unknown_extension() {
        // .bin with TXT content — the dispatcher must fall through to
        // try_all, which picks txt and returns source_format = "txt".
        let dir = unique_dir("unknown_ext");
        let path = dir.join("anything.bin");
        fs::write(&path, "Содержимое без расширения.").unwrap();

        let doc = read_and_parse_file_impl(&path).expect("parse via try_all ok");
        assert_eq!(doc.source_format, "txt");
        assert!(doc.text.contains("без расширения."));

        let _ = fs::remove_dir_all(&dir);
    }
}
