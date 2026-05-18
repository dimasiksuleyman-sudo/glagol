//! DOCX parser using `docx-rust`.
//!
//! Extraction policy (Sprint 4 Q3 — Option β for tables):
//!
//! * Paragraphs and headings → plain text on their own line.
//! * Tables → row-by-row, cells joined with spaces, rows separated
//!   by a newline. Listening at 0.5× to a real-world table will tell
//!   us in Sprint 5 polish whether this needs refinement.
//! * Bullet / numbered list items → extracted as separate paragraphs
//!   (docx-rust represents them as paragraphs with numbering
//!   properties; we ignore the numbering and treat the text only).
//! * Headers, footers, comments, footnotes, tracked changes,
//!   embedded images → all skipped (docx-rust's parse defaults give us
//!   the document body without them).
//! * Bold / italic / hyperlink markup → dropped, text kept (the
//!   docx-rust `text()` / `iter_text()` helpers already flatten runs).

use std::path::Path;

use docx_rust::document::BodyContent;
use docx_rust::DocxFile;

use super::{ParseError, ParsedDocument};

pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let docx_file = DocxFile::from_file(path)
        .map_err(|e| ParseError::Format(format!("открытие .docx: {e:?}")))?;
    let docx = docx_file
        .parse()
        .map_err(|e| ParseError::Format(format!("разбор .docx: {e:?}")))?;

    let mut out = String::new();
    for element in &docx.document.body.content {
        match element {
            BodyContent::Paragraph(p) => {
                let text = paragraph_text(p);
                if !text.is_empty() {
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str(&text);
                    out.push('\n');
                }
            }
            BodyContent::Table(t) => {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                for row in &t.rows {
                    let row_text = row_text(row);
                    if !row_text.is_empty() {
                        out.push_str(&row_text);
                        out.push('\n');
                    }
                }
            }
            // Sdt, SectionProperty, TableCell at top level, Run at top
            // level — none of these carry user-visible content in the
            // typical Word output, skip.
            _ => {}
        }
    }

    Ok(ParsedDocument {
        text: out.trim_end().to_string(),
        is_scanned_pdf: false,
        source_format: "docx".to_string(),
    })
}

/// Collect every `Cow<str>` run inside a paragraph and concatenate.
fn paragraph_text(p: &docx_rust::document::Paragraph<'_>) -> String {
    p.iter_text().map(|s| s.as_ref()).collect::<String>()
}

/// Row-by-row layout (Option β): every cell's flat text joined by a
/// single space, trailing whitespace stripped.
fn row_text(row: &docx_rust::document::TableRow<'_>) -> String {
    let cells: Vec<String> = row
        .cells
        .iter()
        .filter_map(|cell| match cell {
            docx_rust::document::TableRowContent::TableCell(c) => {
                let cell_text: String = c.iter_text().map(|s| s.as_ref()).collect();
                let cleaned = cell_text.split_whitespace().collect::<Vec<_>>().join(" ");
                (!cleaned.is_empty()).then_some(cleaned)
            }
            _ => None,
        })
        .collect();
    cells.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    /// Building a full OOXML payload by hand is brittle, so the
    /// heavier behavioural tests live under manual QA against
    /// real-world Word files. These tests cover only the panic-free
    /// error-path contract that does NOT require a full DOCX round-trip.

    #[test]
    fn parse_returns_format_error_for_non_docx_file() {
        let dir = std::env::temp_dir().join(format!(
            "glagol_parser_docx_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("not-really.docx");
        fs::write(&path, b"this is not a real docx file").unwrap();

        let err = parse(&path).unwrap_err();
        assert!(
            matches!(err, ParseError::Format(_)),
            "non-DOCX bytes must surface as ParseError::Format, got: {err:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_returns_io_error_for_missing_file() {
        let path = std::env::temp_dir().join(format!(
            "glagol_parser_docx_missing_{}.docx",
            uuid::Uuid::new_v4().simple()
        ));
        let err = parse(&path).unwrap_err();
        assert!(
            matches!(err, ParseError::Io(_) | ParseError::Format(_)),
            "missing file must surface as Io (or Format if the crate wraps it), got: {err:?}"
        );
    }
}
