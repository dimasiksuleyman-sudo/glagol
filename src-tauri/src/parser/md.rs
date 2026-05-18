//! Markdown parser using `pulldown-cmark` event streams.
//!
//! Conservative extraction policy (Sprint 4 Q5):
//!
//! * Headings, paragraphs, blockquotes, list items → plain text,
//!   separated by blank lines.
//! * Bold / italic / inline code / strikethrough → markup stripped,
//!   text kept.
//! * Links → link text kept, URL dropped.
//! * **Image alt text** → dropped entirely (silence is better than an
//!   alt suffix that the narrator awkwardly reads aloud).
//! * **Code blocks** → replaced with the placeholder «фрагмент кода».
//! * Tables → row-by-row, cells joined with spaces.
//! * Horizontal rules and raw HTML → dropped.
//! * **Footnotes** → collected and appended at the end under a
//!   «Сноски:» heading.
//!
//! The footnote handling is the only multi-pass element: definitions
//! arrive interleaved with body text in the event stream, so we
//! accumulate them in a separate buffer and stitch the suffix on at
//! the very end. Inline footnote references (`[^1]`) are dropped from
//! the body — the listener gets the full text in the trailing
//! «Сноски» section.

use std::fs;
use std::path::Path;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use super::{ParseError, ParsedDocument};

const CODE_BLOCK_PLACEHOLDER: &str = "фрагмент кода";
const FOOTNOTES_HEADING: &str = "Сноски:";

pub fn parse(path: &Path) -> Result<ParsedDocument, ParseError> {
    let raw = fs::read_to_string(path)?;
    let text = extract(&raw);
    Ok(ParsedDocument {
        text,
        is_scanned_pdf: false,
        source_format: "md".to_string(),
    })
}

pub(crate) fn extract(markdown: &str) -> String {
    let options =
        Options::ENABLE_FOOTNOTES | Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;

    let mut body = String::new();
    let mut footnotes: Vec<(String, String)> = Vec::new();

    // Mutually exclusive context flags. `image_depth` is a counter
    // rather than a bool to be defensive against nested image syntax.
    let mut image_depth: u32 = 0;
    let mut in_code_block = false;
    let mut current_footnote_label: Option<String> = None;
    let mut current_footnote_body = String::new();
    // Tables — row buffer is flushed on TableRow end.
    let mut in_table = false;
    let mut current_row_cells: Vec<String> = Vec::new();
    let mut current_cell_text = String::new();
    let mut in_table_cell = false;

    let parser = Parser::new_ext(markdown, options);

    for event in parser {
        match event {
            // ── Block starts ────────────────────────────────────────
            Event::Start(Tag::CodeBlock(_)) => {
                in_code_block = true;
                if current_footnote_label.is_some() {
                    push_with_space(&mut current_footnote_body, CODE_BLOCK_PLACEHOLDER);
                } else if in_table_cell {
                    push_with_space(&mut current_cell_text, CODE_BLOCK_PLACEHOLDER);
                } else {
                    ensure_paragraph_break(&mut body);
                    body.push_str(CODE_BLOCK_PLACEHOLDER);
                }
            }
            Event::Start(Tag::Image { .. }) => {
                image_depth += 1;
            }
            Event::Start(Tag::FootnoteDefinition(label)) => {
                current_footnote_label = Some(label.to_string());
                current_footnote_body.clear();
            }
            Event::Start(Tag::Table(_)) => {
                in_table = true;
            }
            Event::Start(Tag::TableRow) | Event::Start(Tag::TableHead) => {
                current_row_cells.clear();
            }
            Event::Start(Tag::TableCell) => {
                in_table_cell = true;
                current_cell_text.clear();
            }

            // ── Block ends ──────────────────────────────────────────
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
            }
            Event::End(TagEnd::Image) => {
                image_depth = image_depth.saturating_sub(1);
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                if let Some(label) = current_footnote_label.take() {
                    footnotes.push((label, current_footnote_body.trim().to_string()));
                    current_footnote_body.clear();
                }
            }
            Event::End(TagEnd::Table) => {
                in_table = false;
            }
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                if !current_row_cells.is_empty() {
                    ensure_line_break(&mut body);
                    body.push_str(&current_row_cells.join(" "));
                }
                current_row_cells.clear();
            }
            Event::End(TagEnd::TableCell) => {
                in_table_cell = false;
                current_row_cells.push(std::mem::take(&mut current_cell_text));
            }
            Event::End(TagEnd::Paragraph)
            | Event::End(TagEnd::Heading(_))
            | Event::End(TagEnd::Item)
            | Event::End(TagEnd::BlockQuote(_)) => {
                if current_footnote_label.is_some() {
                    if !current_footnote_body.is_empty() && !current_footnote_body.ends_with(' ') {
                        current_footnote_body.push(' ');
                    }
                } else if !in_table {
                    ensure_paragraph_break(&mut body);
                }
            }

            // ── Text-like events ───────────────────────────────────
            Event::Text(text) | Event::Code(text) => {
                if image_depth > 0 || in_code_block {
                    continue;
                }
                if let Some(_label) = &current_footnote_label {
                    current_footnote_body.push_str(&text);
                } else if in_table_cell {
                    current_cell_text.push_str(&text);
                } else {
                    body.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if image_depth > 0 || in_code_block {
                    continue;
                }
                if current_footnote_label.is_some() {
                    current_footnote_body.push(' ');
                } else if in_table_cell {
                    current_cell_text.push(' ');
                } else {
                    body.push(' ');
                }
            }

            // Everything else (Rule, Html, FootnoteReference, math, …)
            // is intentionally dropped.
            _ => {}
        }
    }

    let body_trimmed = body.trim_end();
    if footnotes.is_empty() {
        return body_trimmed.to_string();
    }

    let mut out = String::with_capacity(body_trimmed.len() + 64);
    out.push_str(body_trimmed);
    out.push_str("\n\n");
    out.push_str(FOOTNOTES_HEADING);
    for (label, text) in footnotes {
        out.push('\n');
        out.push_str(&label);
        out.push_str(". ");
        out.push_str(&text);
    }
    out
}

fn ensure_paragraph_break(s: &mut String) {
    if s.is_empty() {
        return;
    }
    while s.ends_with(' ') {
        s.pop();
    }
    if !s.ends_with("\n\n") {
        if s.ends_with('\n') {
            s.push('\n');
        } else {
            s.push_str("\n\n");
        }
    }
}

fn ensure_line_break(s: &mut String) {
    if s.is_empty() {
        return;
    }
    while s.ends_with(' ') {
        s.pop();
    }
    if !s.ends_with('\n') {
        s.push('\n');
    }
}

fn push_with_space(s: &mut String, addition: &str) {
    if !s.is_empty() && !s.ends_with(' ') {
        s.push(' ');
    }
    s.push_str(addition);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_strips_basic_formatting() {
        let md = "# Заголовок\n\nЭто *курсив* и **жирный** текст.";
        let out = extract(md);
        assert!(out.contains("Заголовок"));
        assert!(out.contains("Это курсив и жирный текст."));
    }

    #[test]
    fn extract_replaces_code_block_with_placeholder() {
        let md = "До блока.\n\n```rust\nfn main() {}\n```\n\nПосле блока.";
        let out = extract(md);
        assert!(out.contains("До блока."));
        assert!(out.contains(CODE_BLOCK_PLACEHOLDER));
        assert!(!out.contains("fn main"));
        assert!(out.contains("После блока."));
    }

    #[test]
    fn extract_drops_image_alt_text_entirely() {
        let md = "Подпись: ![Кот в коробке](cat.png) — окончание.";
        let out = extract(md);
        assert!(!out.contains("Кот в коробке"));
        assert!(out.contains("Подпись:"));
        assert!(out.contains("— окончание."));
    }

    #[test]
    fn extract_keeps_link_text_drops_url() {
        let md = "См. [нашу документацию](https://example.com/docs).";
        let out = extract(md);
        assert!(out.contains("нашу документацию"));
        assert!(!out.contains("example.com"));
        assert!(!out.contains("https://"));
    }

    #[test]
    fn extract_appends_footnotes_section_when_present() {
        let md = "Основной текст[^a].\n\n[^a]: Пояснение к сноске.";
        let out = extract(md);
        assert!(out.contains(FOOTNOTES_HEADING));
        assert!(out.contains("Пояснение к сноске."));
        // No footnotes heading when there are none.
        let plain = extract("Просто текст без сносок.");
        assert!(!plain.contains(FOOTNOTES_HEADING));
    }

    #[test]
    fn extract_emits_table_rows_with_space_joined_cells() {
        let md = "| Имя | Возраст |\n|---|---|\n| Аня | 30 |\n| Боря | 25 |";
        let out = extract(md);
        assert!(out.contains("Имя Возраст"));
        assert!(out.contains("Аня 30"));
        assert!(out.contains("Боря 25"));
    }

    #[test]
    fn extract_handles_lists_as_paragraphs() {
        let md = "- первый\n- второй\n- третий";
        let out = extract(md);
        for item in ["первый", "второй", "третий"] {
            assert!(out.contains(item), "missing list item {item} in {out:?}");
        }
    }
}
