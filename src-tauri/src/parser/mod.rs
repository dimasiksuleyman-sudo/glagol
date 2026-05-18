//! File parsers for the Synthesize page file picker.
//!
//! Each submodule exposes a single `parse(&Path) -> Result<ParsedDocument, ParseError>`
//! function. The dispatcher in [`commands::file`](crate::commands::file)
//! routes by file extension and falls back to [`try_all`] when the
//! extension is unknown (the "Все файлы" escape hatch).
//!
//! Extraction policy (Sprint 4 conservative defaults — see CLAUDE.md
//! Working Agreements for rationale):
//!
//! | Format | Policy |
//! |---|---|
//! | TXT  | BOM detect → UTF-8 strict → Windows-1251 fallback |
//! | MD   | `pulldown-cmark` events; code blocks → «фрагмент кода»; image alt dropped; footnotes appended at end |
//! | DOCX | paragraph + table extraction; tables row-by-row, cells space-joined; headers/footers/comments skipped |
//! | PDF  | `pdfium-render`; scanned PDFs (empty extract) flagged via `is_scanned_pdf = true` |

use std::fmt;
use std::path::Path;

use serde::Serialize;

pub mod docx;
pub mod md;
pub mod pdf;
pub mod txt;

/// Outcome of a successful parse. Returned across the IPC boundary
/// to the frontend.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ParsedDocument {
    /// Extracted plain text. May be empty when `is_scanned_pdf` is true.
    pub text: String,
    /// `true` only for PDFs whose page-text extraction returned nothing
    /// usable (typical of image-only scanned documents). The frontend
    /// uses this flag to show the OCR disclaimer instead of loading an
    /// empty textarea.
    pub is_scanned_pdf: bool,
    /// Format the parser was selected as. `"txt" | "md" | "docx" | "pdf"`.
    pub source_format: String,
}

/// Errors surfaced by the parser layer. Converted to `String` at the
/// Tauri command boundary.
#[derive(Debug)]
pub enum ParseError {
    /// Filesystem read failed (missing, permission denied, etc.).
    Io(std::io::Error),
    /// File contents were not valid for the chosen parser
    /// (corrupt DOCX zip, malformed PDF, etc.).
    Format(String),
    /// TXT decoding failed for every encoding we attempted.
    Encoding(String),
    /// Every parser returned an error during the `try_all` escape hatch.
    AllParsersFailed,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Io(e) => write!(f, "не удалось прочитать файл: {e}"),
            ParseError::Format(msg) => write!(f, "не удалось разобрать файл: {msg}"),
            ParseError::Encoding(msg) => write!(f, "не удалось определить кодировку: {msg}"),
            ParseError::AllParsersFailed => {
                write!(
                    f,
                    "формат файла не распознан (попробуйте сохранить в TXT/MD/DOCX/PDF)"
                )
            }
        }
    }
}

impl std::error::Error for ParseError {}

impl From<std::io::Error> for ParseError {
    fn from(e: std::io::Error) -> Self {
        ParseError::Io(e)
    }
}

/// Try every parser in sequence (txt → md → docx → pdf) and return
/// the first successful result. Used by the dispatcher when the file
/// extension is unknown or unsupported.
pub fn try_all(path: &Path) -> Result<ParsedDocument, ParseError> {
    if let Ok(doc) = txt::parse(path) {
        return Ok(doc);
    }
    if let Ok(doc) = md::parse(path) {
        return Ok(doc);
    }
    if let Ok(doc) = docx::parse(path) {
        return Ok(doc);
    }
    if let Ok(doc) = pdf::parse(path) {
        return Ok(doc);
    }
    Err(ParseError::AllParsersFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn try_all_picks_txt_for_plain_utf8_file() {
        // Plain text decodes cleanly via the txt parser, so try_all
        // should short-circuit on the first attempt and return a
        // "txt"-tagged ParsedDocument.
        let dir =
            std::env::temp_dir().join(format!("glagol_try_all_{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("anything.bin");
        fs::write(&path, "Это просто текст без расширения.").unwrap();

        let doc = try_all(&path).expect("txt parser must accept plain UTF-8");
        assert_eq!(doc.source_format, "txt");
        assert!(doc.text.contains("просто текст"));
        assert!(!doc.is_scanned_pdf);

        let _ = fs::remove_dir_all(&dir);
    }
}
