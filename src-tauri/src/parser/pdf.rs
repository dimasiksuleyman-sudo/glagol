//! PDF parser using `pdfium-render`.
//!
//! The Pdfium shared library (`pdfium.dll` / `libpdfium.so` /
//! `libpdfium.dylib`) is downloaded by `build.rs` from
//! <https://github.com/bblanchon/pdfium-binaries> and laid out in two
//! places so both dev and release installer builds can find it:
//!
//! * `OUT_DIR/pdfium/<lib_name>` — absolute path baked in at compile
//!   time via the `PDFIUM_LIBRARY_PATH` env var; consumed in dev
//!   builds where the binary runs from `target/<profile>/`.
//! * `src-tauri/resources/<lib_name>` — picked up by Tauri's
//!   `bundle.resources` and shipped into the NSIS installer at
//!   `$INSTDIR/resources/<lib_name>`; consumed in release builds.
//!
//! At runtime we try the locations in order and fall back to the
//! system library as a last resort:
//!
//! 1. `<exe_dir>/resources/<lib_name>` — release installer layout.
//! 2. `<exe_dir>/<lib_name>` — alternative install layout (user
//!    relocated, sideloaded build, etc.).
//! 3. `env!("PDFIUM_LIBRARY_PATH")` — dev build absolute path.
//! 4. [`Pdfium::bind_to_system_library`] — final fallback if the user
//!    has Pdfium installed system-wide.
//!
//! Failure to bind surfaces as [`ParseError::Format`] — never a panic.
//!
//! Report empty text layers, failed pages and suspicious character mappings
//! separately. Import the available text in page order.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, MutexGuard};

use pdfium_render::prelude::*;

use super::{ParseError, ParsedDocument, PdfWarnings};

/// Absolute path baked in at build time. Always points to the
/// dev-build cache location; used as the third-tier fallback after
/// the two installer-relative candidates fail.
const PDFIUM_LIBRARY_PATH_BUILD: &str = env!("PDFIUM_LIBRARY_PATH");

/// Platform-specific filename of the Pdfium shared library.
#[cfg(target_os = "windows")]
const PDFIUM_LIB_NAME: &str = "pdfium.dll";
#[cfg(target_os = "macos")]
const PDFIUM_LIB_NAME: &str = "libpdfium.dylib";
#[cfg(all(unix, not(target_os = "macos")))]
const PDFIUM_LIB_NAME: &str = "libpdfium.so";

/// Bind once and keep Pdfium document lifetimes from overlapping. The
/// native library shares internal state even when the wrapper is Send + Sync.
static PDFIUM: LazyLock<Result<Mutex<Pdfium>, String>> = LazyLock::new(|| {
    let bindings = bind_pdfium().map_err(|e| format!("не удалось загрузить Pdfium: {e}"))?;
    Ok(Mutex::new(Pdfium::new(bindings)))
});

fn pdfium() -> Result<MutexGuard<'static, Pdfium>, ParseError> {
    PDFIUM
        .as_ref()
        .map_err(|error| ParseError::Format(error.clone()))?
        .lock()
        .map_err(|_| ParseError::Format("библиотека Pdfium недоступна".into()))
}

/// Try the candidate library locations in order; return the first
/// successful binding. The error returned only describes the last
/// attempt — every prior attempt is silently swallowed because the
/// fallback chain is normal operation, not an error condition.
fn bind_pdfium() -> Result<Box<dyn PdfiumLibraryBindings>, PdfiumError> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from));

    let mut candidates: Vec<PathBuf> = Vec::with_capacity(3);
    if let Some(dir) = &exe_dir {
        candidates.push(dir.join("resources").join(PDFIUM_LIB_NAME));
        candidates.push(dir.join(PDFIUM_LIB_NAME));
    }
    candidates.push(PathBuf::from(PDFIUM_LIBRARY_PATH_BUILD));

    let mut last_err = None;
    for path in candidates {
        if !path.exists() {
            continue;
        }
        match Pdfium::bind_to_library(&path) {
            Ok(b) => return Ok(b),
            Err(e) => last_err = Some(e),
        }
    }

    // Final fallback: a system-wide Pdfium install (e.g. for the user
    // who packaged Pdfium themselves outside our installer).
    Pdfium::bind_to_system_library().map_err(|e| last_err.unwrap_or(e))
}

pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let bytes = fs::read(path)?;
    extract_from_bytes(&bytes)
}

pub(crate) fn extract_from_bytes(bytes: &[u8]) -> Result<ParsedDocument, ParseError> {
    let pdfium = pdfium()?;

    let document = pdfium
        .load_pdf_from_byte_slice(bytes, None)
        .map_err(|error| match error {
            PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => {
                ParseError::PdfPasswordRequired
            }
            other => ParseError::Format(format!("открытие PDF: {other}")),
        })?;

    let mut buffer = String::new();
    let mut warnings = PdfWarnings::default();
    let pages = document.pages();
    // PdfPagesIterator turns a failed page into None, ending the loop early.
    for index in pages.as_range() {
        let page_text = pages.get(index).and_then(|page| {
            let text = page.text()?;
            if text.len() < 0 {
                return Err(PdfiumError::PdfiumLibraryInternalError(
                    PdfiumInternalError::Unknown,
                ));
            }
            Ok(text.all())
        });
        let Ok(page_text) = page_text else {
            warnings.skipped_pages += 1;
            continue;
        };
        if page_text.trim().is_empty() {
            warnings.pages_without_text += 1;
            continue;
        }
        warnings.suspect_font |= suspect_font(&page_text);
        if !buffer.is_empty() && !buffer.ends_with('\n') {
            buffer.push('\n');
        }
        buffer.push_str(&page_text);
    }

    let is_scanned =
        buffer.is_empty() && (warnings.skipped_pages == 0 || warnings.pages_without_text > 0);
    Ok(ParsedDocument {
        text: buffer.trim().to_string(),
        is_scanned_pdf: is_scanned,
        source_format: "pdf".to_string(),
        pdf_warnings: (warnings != PdfWarnings::default()).then_some(warnings),
    })
}

fn suspect_font(text: &str) -> bool {
    let mut total = 0;
    let mut suspicious = 0;
    for ch in text.chars().filter(|ch| !ch.is_whitespace()) {
        total += 1;
        if matches!(ch, '\u{fffd}' | '\u{e000}'..='\u{f8ff}' | '\u{f0000}'..='\u{ffffd}' | '\u{100000}'..='\u{10fffd}')
        {
            suspicious += 1;
        }
    }
    // Measure per page so a single corrupt font is not diluted by other pages.
    suspicious > total / 5
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

    fn stream(content: &str) -> String {
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        )
    }

    fn synthetic_pdf(objects: &[String], trailer_fields: &str) -> Vec<u8> {
        use std::fmt::Write as _;

        let mut file = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (index, object) in objects.iter().enumerate() {
            offsets.push(file.len());
            writeln!(file, "{} 0 obj\n{object}\nendobj", index + 1).unwrap();
        }
        let xref = file.len();
        let size = objects.len() + 1;
        write!(file, "xref\n0 {size}\n0000000000 65535 f \n").unwrap();
        for offset in offsets {
            writeln!(file, "{offset:010} 00000 n ").unwrap();
        }
        write!(
            file,
            "trailer\n<< /Size {size} /Root 1 0 R {trailer_fields} >>\nstartxref\n{xref}\n%%EOF\n"
        )
        .unwrap();
        file.into_bytes()
    }

    // None is a missing page object; Some("") is a page with no text layer.
    fn with_pages(page_texts: &[Option<&str>], cmap: Option<&str>) -> Vec<u8> {
        let mut objects = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".into(),
            String::new(),
            format!(
                "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica {} >>",
                if cmap.is_some() {
                    "/ToUnicode 4 0 R"
                } else {
                    ""
                }
            ),
            stream(cmap.unwrap_or_default()),
        ];
        let mut kids = Vec::new();
        for text in page_texts {
            let Some(text) = text else {
                kids.push("99999 0 R".into());
                continue;
            };
            let page = objects.len() + 1;
            let contents = page + 1;
            kids.push(format!("{page} 0 R"));
            objects.push(format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
                 /Resources << /Font << /F1 3 0 R >> >> /Contents {contents} 0 R >>"
            ));
            objects.push(stream(&format!("BT /F1 12 Tf 40 720 Td ({text}) Tj ET")));
        }
        objects[1] = format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            page_texts.len()
        );
        synthetic_pdf(&objects, "")
    }

    #[test]
    fn imports_pages_around_a_missing_page_and_reports_blank_pages() {
        let pdf = with_pages(
            &[Some("First page"), None, Some(""), Some("Last page")],
            None,
        );
        let doc = extract_from_bytes(&pdf).unwrap();
        assert_eq!(doc.text, "First page\nLast page");
        assert!(!doc.is_scanned_pdf);
        assert_eq!(
            doc.pdf_warnings,
            Some(PdfWarnings {
                skipped_pages: 1,
                pages_without_text: 1,
                suspect_font: false,
            })
        );
    }

    #[test]
    fn distinguishes_a_scan_from_a_document_with_no_readable_pages() {
        let scanned = extract_from_bytes(&with_pages(&[Some("")], None)).unwrap();
        assert!(scanned.is_scanned_pdf);
        assert!(scanned.text.is_empty());
        assert_eq!(scanned.pdf_warnings.unwrap().pages_without_text, 1);

        let failed = extract_from_bytes(&with_pages(&[None], None)).unwrap();
        assert!(!failed.is_scanned_pdf);
        assert!(failed.text.is_empty());
        assert_eq!(failed.pdf_warnings.unwrap().skipped_pages, 1);
    }

    #[test]
    fn reports_private_use_characters_from_a_pdf_font_map() {
        let cmap = "/CIDInit /ProcSet findresource begin\n            12 dict begin begincmap\n            /CIDSystemInfo << /Registry (Fixture) /Ordering (UCS) /Supplement 0 >> def\n            /CMapName /Fixture def /CMapType 2 def\n            1 begincodespacerange <00> <FF> endcodespacerange\n            1 beginbfchar <41> <E000> endbfchar\n            endcmap CMapName currentdict /CMap defineresource pop end end";
        let doc = extract_from_bytes(&with_pages(&[Some("AAAA")], Some(cmap))).unwrap();
        assert_eq!(doc.text, "\u{e000}".repeat(4));
        assert!(doc.pdf_warnings.unwrap().suspect_font);
        assert!(!suspect_font("abcd\u{fffd}")); // Exactly 20 percent.
        assert!(suspect_font("abc \u{fffd}"));
        assert!(suspect_font("ab \u{f0000}"));
        assert!(!suspect_font("Обычный текст 123 🙂"));
    }

    #[test]
    fn password_prompt_replaces_pdfium_error_for_an_encrypted_pdf() {
        // Standard PDF encryption dictionary generated for a blank page with
        // pypdf using RC4-40 and fixture passwords. No external file is used.
        let pdf = synthetic_pdf(
            &[
                "<< /Type /Catalog /Pages 2 0 R >>".into(),
                "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>".into(),
                "<< /V 1 /R 2 /Length 40 /P -4 /Filter /Standard \
                 /O <b808eec1af4f505d267712f756ae10835a6150f5a117296d549b2d07ec2d2c00> \
                 /U <bfc2cabda86ee5174411f1a5cdc644e643cb419c1e5c3ede1fe364b61d135854> >>".into(),
            ],
            "/Encrypt 4 0 R /ID [<3561313262376437383561366435353735363962623730643234323261373039> \
                                  <3561313262376437383561366435353735363962623730643234323261373039>]",
        );
        assert!(pdfium()
            .unwrap()
            .load_pdf_from_byte_slice(&pdf, Some("fixture-user"))
            .is_ok());
        let err = extract_from_bytes(&pdf).unwrap_err();
        assert!(matches!(err, ParseError::PdfPasswordRequired));
        assert_eq!(err.to_string(), "Файл защищён паролем");
        assert_eq!(
            crate::i18n::text(&err.to_string(), crate::preferences::Language::En),
            "The file is password protected"
        );
    }
}
