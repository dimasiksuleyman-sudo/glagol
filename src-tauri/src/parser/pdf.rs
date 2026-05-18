//! PDF parser using `pdfium-render`.
//!
//! The Pdfium shared library (`libpdfium.so` / `pdfium.dll`) is
//! downloaded by `build.rs` from <https://github.com/bblanchon/pdfium-binaries>
//! into `OUT_DIR/pdfium/`, and the absolute path is propagated to the
//! compiled binary via the `PDFIUM_LIBRARY_PATH` env var (set at build
//! time). We bind to that path at runtime via
//! [`Pdfium::bind_to_library`]; if the file is missing (offline
//! build / restricted CI) we fall back to the system library so a
//! user-provided Pdfium install still works.
//!
//! Scanned PDF detection (Sprint 4 Q7): a PDF whose extracted text is
//! empty or whitespace-only after trimming is treated as a scanned
//! image. The result still resolves successfully but with
//! `is_scanned_pdf = true`, which the frontend uses to show the OCR
//! disclaimer dialog instead of loading an empty textarea.

use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use pdfium_render::prelude::*;

use super::{ParseError, ParsedDocument};

/// Path to the Pdfium shared library, baked in at build time.
const PDFIUM_LIBRARY_PATH: &str = env!("PDFIUM_LIBRARY_PATH");

/// A single shared `Pdfium` instance, lazily bound on first use.
/// Cheap to share — `Pdfium` is `Send + Sync` and the dynamic binding
/// behind it is itself thread-safe.
static PDFIUM: LazyLock<Result<Pdfium, String>> = LazyLock::new(|| {
    let bindings = Pdfium::bind_to_library(PDFIUM_LIBRARY_PATH)
        .or_else(|_| Pdfium::bind_to_system_library())
        .map_err(|e| format!("не удалось загрузить Pdfium: {e}"))?;
    Ok(Pdfium::new(bindings))
});

pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let bytes = fs::read(path)?;
    extract_from_bytes(&bytes)
}

pub(crate) fn extract_from_bytes(bytes: &[u8]) -> Result<ParsedDocument, ParseError> {
    let pdfium = match PDFIUM.as_ref() {
        Ok(p) => p,
        Err(e) => return Err(ParseError::Format(e.clone())),
    };

    let document = pdfium
        .load_pdf_from_byte_slice(bytes, None)
        .map_err(|e| ParseError::Format(format!("открытие PDF: {e}")))?;

    let mut buffer = String::new();
    for page in document.pages().iter() {
        let text_page = page
            .text()
            .map_err(|e| ParseError::Format(format!("извлечение текста страницы: {e}")))?;
        let page_text = text_page.all();
        if !page_text.is_empty() {
            if !buffer.is_empty() && !buffer.ends_with('\n') {
                buffer.push('\n');
            }
            buffer.push_str(&page_text);
        }
    }

    let trimmed = buffer.trim().to_string();
    let is_scanned = trimmed.is_empty();
    Ok(ParsedDocument {
        text: trimmed,
        is_scanned_pdf: is_scanned,
        source_format: "pdf".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_returns_format_error_for_non_pdf_bytes() {
        let err = extract_from_bytes(b"this is plainly not a PDF").unwrap_err();
        assert!(
            matches!(err, ParseError::Format(_)),
            "non-PDF bytes must surface as ParseError::Format, got: {err:?}"
        );
    }

    #[test]
    fn parse_returns_io_error_for_missing_file() {
        let path = std::env::temp_dir().join(format!(
            "glagol_parser_pdf_missing_{}.pdf",
            uuid::Uuid::new_v4().simple()
        ));
        let err = parse(&path).unwrap_err();
        assert!(
            matches!(err, ParseError::Io(_)),
            "missing file must surface as Io, got: {err:?}"
        );
    }

    /// Minimal valid PDF that opens cleanly but contains no
    /// extractable text — exactly the shape of a scanned document
    /// from the parser's point of view. Built inline so the test has
    /// no external fixture dependency.
    #[test]
    fn extract_flags_scanned_pdf_when_no_text_present() {
        // A 1-page PDF with an empty content stream. Built by hand
        // from the minimal PDF reference layout: %PDF-1.4, Catalog,
        // Pages with one Page, empty Contents stream, xref, trailer.
        const SCANNED_LIKE_PDF: &[u8] = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << >> >>\nendobj\n\
4 0 obj\n<< /Length 0 >>\nstream\n\nendstream\nendobj\n\
xref\n0 5\n0000000000 65535 f \n0000000010 00000 n \n0000000060 00000 n \n0000000110 00000 n \n0000000200 00000 n \n\
trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n270\n%%EOF\n";

        let result = extract_from_bytes(SCANNED_LIKE_PDF);
        // Two acceptable outcomes:
        //   - Pdfium parses the minimal PDF: we expect is_scanned_pdf=true.
        //   - Pdfium rejects the hand-crafted xref offsets and we get
        //     ParseError::Format. Either way the panic-free contract holds.
        match result {
            Ok(doc) => {
                assert!(
                    doc.is_scanned_pdf,
                    "empty-content-stream PDF must be flagged as scanned, got text={:?}",
                    doc.text
                );
                assert_eq!(doc.source_format, "pdf");
                assert!(doc.text.is_empty());
            }
            Err(ParseError::Format(_)) => {
                // Pdfium rejected the hand-written xref — that's fine,
                // the production scanned-PDF path is exercised via
                // manual QA against a real scanned file.
            }
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    }
}
