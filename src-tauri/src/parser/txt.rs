//! Plain-text parser with encoding detection.
//!
//! Detection chain (per Sprint 4 Q4 decision):
//!
//! 1. **BOM** — if the file starts with a recognisable byte-order mark
//!    (`UTF-8`, `UTF-16 LE`, `UTF-16 BE`), trust it and decode with the
//!    matching encoding. Strips the BOM from the returned text.
//! 2. **UTF-8 strict** — `std::str::from_utf8` over the raw bytes.
//!    Catches modern UTF-8 files without a BOM (the common case).
//! 3. **Windows-1251 fallback** — `encoding_rs::WINDOWS_1251.decode`,
//!    used as a last resort to handle legacy Russian `.txt` files
//!    that predate UTF-8 adoption.
//!
//! Anything that still fails after step 3 surfaces as a
//! [`ParseError::Encoding`].

use std::fs;
use std::path::Path;

use encoding_rs::{Encoding, WINDOWS_1251};

use super::{ParseError, ParsedDocument};

pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let bytes = fs::read(path)?;
    let text = decode(&bytes)?;
    Ok(ParsedDocument {
        text,
        is_scanned_pdf: false,
        source_format: "txt".to_string(),
    })
}

pub(crate) fn decode(bytes: &[u8]) -> Result<String, ParseError> {
    // 1. BOM.
    if let Some((encoding, bom_len)) = Encoding::for_bom(bytes) {
        let (decoded, _, had_errors) = encoding.decode(&bytes[bom_len..]);
        if had_errors {
            return Err(ParseError::Encoding(format!(
                "BOM указал {} но содержимое не декодируется",
                encoding.name()
            )));
        }
        return Ok(decoded.into_owned());
    }
    // 2. UTF-8 strict — preferred for modern files without a BOM.
    if let Ok(s) = std::str::from_utf8(bytes) {
        return Ok(s.to_string());
    }
    // 3. Windows-1251 fallback for legacy Russian text.
    let (decoded, _, had_errors) = WINDOWS_1251.decode(bytes);
    if had_errors {
        return Err(ParseError::Encoding(
            "ни UTF-8, ни Windows-1251 не подошли".to_string(),
        ));
    }
    Ok(decoded.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_plain_utf8_without_bom() {
        let bytes = "Привет, мир!".as_bytes();
        assert_eq!(decode(bytes).unwrap(), "Привет, мир!");
    }

    #[test]
    fn decode_utf8_with_bom_strips_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("Привет, мир!".as_bytes());
        assert_eq!(decode(&bytes).unwrap(), "Привет, мир!");
    }

    #[test]
    fn decode_utf16_le_with_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        for cp in "Привет".encode_utf16() {
            bytes.extend_from_slice(&cp.to_le_bytes());
        }
        assert_eq!(decode(&bytes).unwrap(), "Привет");
    }

    #[test]
    fn decode_utf16_be_with_bom() {
        let mut bytes = vec![0xFE, 0xFF];
        for cp in "Привет".encode_utf16() {
            bytes.extend_from_slice(&cp.to_be_bytes());
        }
        assert_eq!(decode(&bytes).unwrap(), "Привет");
    }

    #[test]
    fn decode_windows_1251_fallback() {
        // "Привет" in Windows-1251: each Cyrillic letter is one byte.
        let bytes: &[u8] = &[0xCF, 0xF0, 0xE8, 0xE2, 0xE5, 0xF2];
        assert_eq!(decode(bytes).unwrap(), "Привет");
    }

    #[test]
    fn decode_handles_ascii_via_utf8_branch() {
        assert_eq!(decode(b"Hello, world!").unwrap(), "Hello, world!");
    }

    #[test]
    fn parse_reads_file_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "glagol_parser_txt_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("note.txt");
        fs::write(&path, "Простой текст.\nВторая строка.").unwrap();

        let doc = parse(&path).expect("parse ok");
        assert_eq!(doc.text, "Простой текст.\nВторая строка.");
        assert!(!doc.is_scanned_pdf);
        assert_eq!(doc.source_format, "txt");

        let _ = fs::remove_dir_all(&dir);
    }
}
