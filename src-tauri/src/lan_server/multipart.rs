//! A small, bounded `multipart/form-data` parser for one uploaded file and a
//! few text fields — exactly what OpenAI-compatible transcription clients send.

use std::collections::HashMap;

/// At most this many parts are accepted.
const MAX_PARTS: usize = 16;
/// A text field longer than this is rejected.
pub const MAX_FIELD_BYTES: usize = 4096;
/// The header block of one part is at most this long.
const MAX_PART_HEADER_BYTES: usize = 2048;

/// The parsed form.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Form {
    /// Bytes of the part named `file`.
    pub file: Vec<u8>,
    /// Other text fields by name (`model`, `language`, `response_format`, …).
    pub fields: HashMap<String, String>,
}

/// Why a body was rejected; the message goes to the client.
#[derive(Debug, PartialEq, Eq)]
pub struct MultipartError(pub &'static str);

/// The `boundary` parameter of a `multipart/form-data` content type.
pub fn boundary(content_type: &str) -> Option<String> {
    let mut parts = content_type.split(';');
    if !parts
        .next()?
        .trim()
        .eq_ignore_ascii_case("multipart/form-data")
    {
        return None;
    }
    parts.find_map(|p| {
        let (name, value) = p.split_once('=')?;
        if !name.trim().eq_ignore_ascii_case("boundary") {
            return None;
        }
        let value = value.trim().trim_matches('"');
        (!value.is_empty() && value.len() <= 70).then(|| value.to_string())
    })
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    let first = needle[0];
    let last_start = haystack.len() - needle.len();
    let mut i = from;
    while i <= last_start {
        let at = i + haystack[i..=last_start].iter().position(|&b| b == first)?;
        if &haystack[at..at + needle.len()] == needle {
            return Some(at);
        }
        i = at + 1;
    }
    None
}

/// The `name` parameter of a part's `Content-Disposition: form-data` header.
fn part_name(headers: &str) -> Option<String> {
    headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if !name.trim().eq_ignore_ascii_case("content-disposition") {
            return None;
        }
        value.split(';').find_map(|p| {
            let (k, v) = p.split_once('=')?;
            k.trim()
                .eq_ignore_ascii_case("name")
                .then(|| v.trim().trim_matches('"').to_string())
        })
    })
}

/// Parse `body` with `boundary`. Requires exactly one part named `file`.
pub fn parse(body: &[u8], boundary: &str) -> Result<Form, MultipartError> {
    let delimiter = format!("--{boundary}").into_bytes();
    let next_delimiter = format!("\r\n--{boundary}").into_bytes();
    let start = find(body, &delimiter, 0).ok_or(MultipartError("missing multipart boundary"))?;
    let mut pos = start + delimiter.len();
    let mut form = Form::default();
    let mut has_file = false;
    for _ in 0..=MAX_PARTS {
        // After a delimiter: "--" ends the body, CRLF starts a part.
        if body[pos..].starts_with(b"--") {
            if !has_file {
                return Err(MultipartError("missing file field"));
            }
            return Ok(form);
        }
        if !body[pos..].starts_with(b"\r\n") {
            return Err(MultipartError("malformed multipart body"));
        }
        pos += 2;
        let header_end =
            find(body, b"\r\n\r\n", pos).ok_or(MultipartError("malformed part headers"))?;
        if header_end - pos > MAX_PART_HEADER_BYTES {
            return Err(MultipartError("part headers too long"));
        }
        let headers = std::str::from_utf8(&body[pos..header_end])
            .map_err(|_| MultipartError("part headers are not UTF-8"))?;
        let name = part_name(headers).ok_or(MultipartError("part without a name"))?;
        let data_start = header_end + 4;
        let data_end = find(body, &next_delimiter, data_start)
            .ok_or(MultipartError("unterminated multipart part"))?;
        let data = &body[data_start..data_end];
        if name == "file" {
            if has_file {
                return Err(MultipartError("more than one file"));
            }
            has_file = true;
            form.file = data.to_vec();
        } else {
            if data.len() > MAX_FIELD_BYTES {
                return Err(MultipartError("form field too long"));
            }
            let value =
                std::str::from_utf8(data).map_err(|_| MultipartError("form field is not UTF-8"))?;
            form.fields.insert(name, value.to_string());
        }
        pos = data_end + next_delimiter.len();
    }
    Err(MultipartError("too many multipart parts"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(parts: &[(&str, Option<&str>, &[u8])], boundary: &str) -> Vec<u8> {
        let mut out = Vec::new();
        for (name, filename, data) in parts {
            out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            match filename {
                Some(f) => out.extend_from_slice(
                    format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{f}\"\r\nContent-Type: audio/wav\r\n\r\n").as_bytes(),
                ),
                None => out.extend_from_slice(
                    format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
                ),
            }
            out.extend_from_slice(data);
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        out
    }

    #[test]
    fn boundary_is_read_from_the_content_type() {
        assert_eq!(
            boundary("multipart/form-data; boundary=abc123").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            boundary("Multipart/Form-Data; charset=utf-8; boundary=\"x y\"").as_deref(),
            Some("x y")
        );
        assert_eq!(boundary("application/json"), None);
        assert_eq!(boundary("multipart/form-data"), None);
    }

    #[test]
    fn parses_file_and_fields_with_binary_content() {
        let audio: &[u8] = b"RIFF\r\n--not-the-boundary\x00\xff";
        let b = body(
            &[
                ("file", Some("audio.wav"), audio),
                ("model", None, b"gigaam-v3-e2e-ctc"),
                ("language", None, b"ru"),
                ("response_format", None, b"json"),
            ],
            "BOUNDARY42",
        );
        let form = parse(&b, "BOUNDARY42").unwrap();
        assert_eq!(form.file, audio);
        assert_eq!(form.fields["model"], "gigaam-v3-e2e-ctc");
        assert_eq!(form.fields["language"], "ru");
        assert_eq!(form.fields.len(), 3);
    }

    #[test]
    fn rejects_malformed_bodies() {
        let ok = body(&[("file", Some("a.wav"), b"x")], "B");
        assert!(parse(&ok, "B").is_ok());
        // Missing file.
        let no_file = body(&[("model", None, b"m")], "B");
        assert_eq!(
            parse(&no_file, "B"),
            Err(MultipartError("missing file field"))
        );
        // Two files.
        let two = body(&[("file", Some("a"), b"x"), ("file", Some("b"), b"y")], "B");
        assert!(parse(&two, "B").is_err());
        // Wrong boundary, truncated body, oversized field, garbage.
        assert!(parse(&ok, "C").is_err());
        assert!(parse(&ok[..ok.len() - 12], "B").is_err());
        let long = vec![b'a'; MAX_FIELD_BYTES + 1];
        let big = body(&[("file", Some("a"), b"x"), ("prompt", None, &long)], "B");
        assert_eq!(parse(&big, "B"), Err(MultipartError("form field too long")));
        assert!(parse(b"--B\r\nno headers end", "B").is_err());
        assert!(parse(b"", "B").is_err());
        let many: Vec<(&str, Option<&str>, &[u8])> =
            (0..20).map(|_| ("x", None, b"1" as &[u8])).collect();
        assert!(parse(&body(&many, "B"), "B").is_err());
    }
}
