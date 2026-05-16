//! Text chunking for SaluteSpeech sync API.
//!
//! SaluteSpeech sync `/text:synthesize` accepts up to 4000 chars per request.
//! [`chunk_text`] splits arbitrary UTF-8 text into chunks small enough for
//! the API, preferring natural boundaries (paragraphs > sentences > words).
//!
//! # Strategy
//!
//! 1. **Paragraph boundaries** (`\n\n+`) — each paragraph becomes its own
//!    chunk by default. As an exception, a short paragraph (< 100 chars)
//!    that does NOT end in a terminator (`.`, `!`, `?`, `…`) is merged
//!    with the next paragraph if the combined size fits. This keeps short
//!    headings attached to their content instead of one-line API calls.
//! 2. **Sentence ends** — `[.!?…]` (or `.` followed by more `.`s, treated
//!    as `...`) followed by whitespace and an uppercase letter. Used
//!    inside a single oversized paragraph.
//! 3. **Word boundaries** — fallback for a single sentence that is itself
//!    longer than `max_chars`.
//! 4. **Hard cut** — last resort for a single token longer than `max_chars`
//!    (e.g. a 4500-char URL in a PDF). Logged via [`tracing::warn`].
//!
//! All lengths are measured in `chars`, not bytes. All slicing uses
//! [`str::char_indices`], so multi-byte characters (Cyrillic, emoji)
//! are never split mid-byte.
//!
//! # Out of scope
//!
//! This module does NOT preprocess the input: it does not replace URLs,
//! decode HTML entities, normalize ellipses, expand abbreviations, or
//! strip Markdown formatting. See `text::preprocessor` (planned Sprint 3,
//! tracked in a GitHub issue) for those transformations. The preprocessor
//! is meant to run BEFORE `chunk_text` in the pipeline.

/// Default chunk size for the SaluteSpeech sync API.
///
/// 3500 leaves a 500-char safety margin under the 4000-char API limit
/// (room for future SSML overhead and UTF-8 variance in error messages).
pub const DEFAULT_MAX_CHARS: usize = 3500;

/// Threshold under which a paragraph is considered "short" and may be
/// merged forward with the next paragraph (heading-like blocks).
const SHORT_PARAGRAPH_THRESHOLD: usize = 100;

/// Split `text` into chunks, each at most `max_chars` characters long.
///
/// Returns an empty `Vec` for empty / whitespace-only input or for
/// `max_chars == 0`. Each returned chunk has leading and trailing
/// whitespace trimmed.
///
/// See module-level docs for the splitting strategy.
pub fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    if max_chars == 0 || text.trim().is_empty() {
        return Vec::new();
    }

    let paragraphs = split_by_paragraphs(text);
    let mut chunks: Vec<String> = Vec::new();
    let mut i = 0;
    while i < paragraphs.len() {
        let p = paragraphs[i];
        let p_len = p.chars().count();

        // Variant B merge: short, non-terminator-ending paragraph + next.
        if p_len < SHORT_PARAGRAPH_THRESHOLD
            && p_len <= max_chars
            && !paragraph_ends_with_terminator(p)
            && i + 1 < paragraphs.len()
        {
            let next = paragraphs[i + 1];
            let next_len = next.chars().count();
            // "\n\n" between (2 chars) preserves paragraph semantics for
            // the TTS engine (Sberbank renders \n as a brief pause).
            if p_len + 2 + next_len <= max_chars {
                let mut joined = String::with_capacity(p.len() + 2 + next.len());
                joined.push_str(p);
                joined.push_str("\n\n");
                joined.push_str(next);
                chunks.push(joined);
                i += 2;
                continue;
            }
        }

        if p_len <= max_chars {
            chunks.push(p.to_string());
        } else {
            chunks.extend(chunk_paragraph(p, max_chars));
        }
        i += 1;
    }

    chunks
}

/// Returns trimmed, non-empty paragraphs (text between runs of whitespace
/// containing two or more newlines). Robust to mixed `\n` / `\r\n` runs;
/// CR alone is treated as part of a whitespace run, not a paragraph break.
fn split_by_paragraphs(text: &str) -> Vec<&str> {
    let mut paragraphs = Vec::new();
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if matches!(bytes[i], b'\n' | b'\r' | b' ' | b'\t') {
            let run_start = i;
            let mut newlines = 0usize;
            while i < bytes.len() && matches!(bytes[i], b'\n' | b'\r' | b' ' | b'\t') {
                if bytes[i] == b'\n' {
                    newlines += 1;
                }
                i += 1;
            }
            if newlines >= 2 {
                let trimmed = text[start..run_start].trim();
                if !trimmed.is_empty() {
                    paragraphs.push(trimmed);
                }
                start = i;
            }
        } else {
            i += 1;
        }
    }
    if start < bytes.len() {
        let trimmed = text[start..].trim();
        if !trimmed.is_empty() {
            paragraphs.push(trimmed);
        }
    }
    paragraphs
}

/// Heading-detection helper: true iff the trimmed text ends in `.`, `!`, `?`, or `…`.
fn paragraph_ends_with_terminator(p: &str) -> bool {
    p.trim_end()
        .chars()
        .last()
        .map(|c| matches!(c, '.' | '!' | '?' | '…'))
        .unwrap_or(false)
}

fn is_sentence_end_char(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?' | '…')
}

/// Splits one oversized paragraph by sentence boundaries, packing
/// adjacent sentences into chunks of `≤ max_chars` characters.
///
/// Note: multiple consecutive whitespace characters between
/// sentences are normalized to a single space in the output.
/// This is intentional — Sberbank's TTS engine ignores whitespace
/// runs, and normalization keeps a tight bound on chunk lengths.
fn chunk_paragraph(paragraph: &str, max_chars: usize) -> Vec<String> {
    if paragraph.chars().count() <= max_chars {
        return vec![paragraph.trim().to_string()];
    }

    let sentences = split_by_sentences(paragraph);
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;

    for sentence in sentences {
        let s = sentence.trim();
        if s.is_empty() {
            continue;
        }
        let s_len = s.chars().count();

        if s_len > max_chars {
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
                current_len = 0;
            }
            chunks.extend(chunk_by_words(s, max_chars));
            continue;
        }

        let joiner = if current.is_empty() { 0 } else { 1 };
        if current_len + joiner + s_len <= max_chars {
            if !current.is_empty() {
                current.push(' ');
                current_len += 1;
            }
            current.push_str(s);
            current_len += s_len;
        } else {
            chunks.push(std::mem::take(&mut current));
            current.push_str(s);
            current_len = s_len;
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Splits `text` at sentence boundaries.
///
/// A boundary is any of `.`, `!`, `?`, `…` (or a run of `.`s, treated as
/// `...`) immediately followed by ≥1 whitespace char and an uppercase
/// letter. The terminator stays attached to the preceding sentence; the
/// whitespace between sentences is dropped. Returned slices may contain
/// internal whitespace but no leading/trailing whitespace at boundaries.
fn split_by_sentences(text: &str) -> Vec<&str> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.is_empty() {
        return Vec::new();
    }

    let mut sentences = Vec::new();
    let mut start_byte = chars[0].0;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i].1;
        if is_sentence_end_char(ch) {
            // For '.', swallow consecutive '.'s so "..." counts as one terminator run.
            let mut last_term_i = i;
            if ch == '.' {
                while last_term_i + 1 < chars.len() && chars[last_term_i + 1].1 == '.' {
                    last_term_i += 1;
                }
            }

            // After the terminator(s): expect whitespace then uppercase.
            let after_term = last_term_i + 1;
            if after_term < chars.len() && chars[after_term].1.is_whitespace() {
                let mut j = after_term;
                while j < chars.len() && chars[j].1.is_whitespace() {
                    j += 1;
                }
                if j < chars.len() && chars[j].1.is_uppercase() {
                    let (term_byte, term_char) = chars[last_term_i];
                    let term_end_byte = term_byte + term_char.len_utf8();
                    sentences.push(&text[start_byte..term_end_byte]);
                    start_byte = chars[j].0;
                    i = j;
                    continue;
                }
            }
            i = last_term_i + 1;
            continue;
        }
        i += 1;
    }

    if start_byte < text.len() {
        let last = &text[start_byte..];
        if !last.is_empty() {
            sentences.push(last);
        }
    }
    sentences
}

/// Greedy pack `text` (one oversized sentence) into chunks by whitespace
/// boundaries. Falls through to [`hard_cut`] for individual tokens
/// longer than `max_chars`.
fn chunk_by_words(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;

    for word in text.split_whitespace() {
        let w_len = word.chars().count();

        if w_len > max_chars {
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
                current_len = 0;
            }
            chunks.extend(hard_cut(word, max_chars));
            continue;
        }

        let joiner = if current.is_empty() { 0 } else { 1 };
        if current_len + joiner + w_len <= max_chars {
            if !current.is_empty() {
                current.push(' ');
                current_len += 1;
            }
            current.push_str(word);
            current_len += w_len;
        } else {
            chunks.push(std::mem::take(&mut current));
            current.push_str(word);
            current_len = w_len;
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Last-resort splitter: cuts a single oversized token every `max_chars`
/// characters. Iterates by `char`, so multi-byte UTF-8 sequences are
/// never split mid-byte. Logs a warning because the resulting audio
/// will have an unnatural break inside a single word.
fn hard_cut(text: &str, max_chars: usize) -> Vec<String> {
    tracing::warn!(
        chars = text.chars().count(),
        max_chars,
        "hard-cutting a single token longer than max_chars; audio quality may suffer"
    );

    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;
    for ch in text.chars() {
        if current_len == max_chars {
            chunks.push(std::mem::take(&mut current));
            current_len = 0;
        }
        current.push(ch);
        current_len += 1;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    // #1
    #[test]
    fn test_empty_string_returns_empty_vec() {
        assert!(chunk_text("", DEFAULT_MAX_CHARS).is_empty());
    }

    // #2
    #[test]
    fn test_whitespace_only_returns_empty_vec() {
        assert!(chunk_text("   \n  \n  ", DEFAULT_MAX_CHARS).is_empty());
        assert!(chunk_text("\n\n\n", DEFAULT_MAX_CHARS).is_empty());
        assert!(chunk_text("\t\t  \t", DEFAULT_MAX_CHARS).is_empty());
    }

    // #3
    #[test]
    fn test_short_text_returns_single_chunk() {
        assert_eq!(chunk_text("Hello.", DEFAULT_MAX_CHARS), vec!["Hello."]);
    }

    // #4
    #[test]
    fn test_exactly_max_chars_single_chunk() {
        let text: String = "a".repeat(3500);
        let chunks = chunk_text(&text, 3500);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chars().count(), 3500);
    }

    // #5 — both paragraphs end in a terminator, so heading-merge does NOT fire.
    #[test]
    fn test_two_paragraphs_split_by_double_newline() {
        assert_eq!(chunk_text("A.\n\nB.", DEFAULT_MAX_CHARS), vec!["A.", "B."]);
    }

    // #6
    #[test]
    fn test_multiple_paragraphs_with_empty_lines() {
        assert_eq!(
            chunk_text("A.\n\n\n\nB.", DEFAULT_MAX_CHARS),
            vec!["A.", "B."]
        );
        assert_eq!(
            chunk_text("First.\n\n\n\n\nSecond.\n\n\nThird.", DEFAULT_MAX_CHARS),
            vec!["First.", "Second.", "Third."]
        );
    }

    // #7 — two sentences fit individually but not together.
    #[test]
    fn test_long_paragraph_split_by_sentences() {
        let sentence_a = format!("{}.", "a".repeat(2000));
        let sentence_b = format!("{}.", "B".repeat(2000));
        let text = format!("{} {}", sentence_a, sentence_b);
        let chunks = chunk_text(&text, 3500);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].chars().count() <= 3500);
        assert!(chunks[1].chars().count() <= 3500);
        assert!(chunks[0].ends_with('.'));
    }

    // #8 — "т.е." not a boundary: next char is lowercase. Whole paragraph fits.
    #[test]
    fn test_sentence_end_requires_uppercase_next() {
        let chunks = chunk_text("т.е. молочный продукт.", DEFAULT_MAX_CHARS);
        assert_eq!(chunks, vec!["т.е. молочный продукт."]);
    }

    // #9 — one sentence, no terminator, longer than max_chars → split by words.
    #[test]
    fn test_long_sentence_split_by_words() {
        let text = "word ".repeat(100); // 500 chars
        let chunks = chunk_text(text.trim(), 20);
        assert!(chunks.len() >= 2);
        for c in &chunks {
            assert!(c.chars().count() <= 20, "chunk exceeded max_chars: {:?}", c);
            assert!(!c.starts_with(' '));
            assert!(!c.ends_with(' '));
        }
    }

    // #10 — 4500-char token, no whitespace → hard cut at 3500.
    #[test]
    fn test_very_long_word_hard_cut() {
        let word = "a".repeat(4500);
        let chunks = chunk_text(&word, 3500);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].chars().count(), 3500);
        assert_eq!(chunks[1].chars().count(), 1000);
    }

    // #11 — Cyrillic 'я' is 2 bytes; chunker must count chars not bytes.
    #[test]
    fn test_cyrillic_counted_as_chars_not_bytes() {
        let text = "я".repeat(3500);
        assert_eq!(text.len(), 7000);
        let chunks = chunk_text(&text, 3500);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chars().count(), 3500);
    }

    // #12 — emoji on chunk boundary preserved intact.
    #[test]
    fn test_emoji_not_split_in_middle() {
        let text = "abcdefghij👋klm";
        let chunks = chunk_text(text, 10);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "abcdefghij");
        assert_eq!(chunks[1], "👋klm");
        assert!(chunks.iter().any(|c| c.contains('👋')));
    }

    // #13 — chunk trim removes leading / trailing whitespace.
    #[test]
    fn test_chunk_trim_removes_leading_trailing_whitespace() {
        assert_eq!(chunk_text("  text  ", DEFAULT_MAX_CHARS), vec!["text"]);
        assert_eq!(
            chunk_text("\n\n  text  \n\n", DEFAULT_MAX_CHARS),
            vec!["text"]
        );
    }

    // #13b — Variant B: short paragraph without terminator merges forward.
    #[test]
    fn test_short_heading_merges_with_next_paragraph() {
        assert_eq!(
            chunk_text("H1\n\nThis is content.", DEFAULT_MAX_CHARS),
            vec!["H1\n\nThis is content."]
        );
    }

    // #13c — Variant B: short paragraph WITH terminator does NOT merge.
    #[test]
    fn test_short_paragraph_with_terminator_does_not_merge() {
        assert_eq!(chunk_text("A.\n\nB.", DEFAULT_MAX_CHARS), vec!["A.", "B."]);
    }

    // #14 — all terminators (`.`, `!`, `?`, `…`, `...`) work as boundaries.
    #[test]
    fn test_all_sentence_terminators() {
        assert_eq!(
            split_by_sentences("One. Two! Three? Four… Five"),
            vec!["One.", "Two!", "Three?", "Four…", "Five"]
        );
        assert_eq!(
            split_by_sentences("Привет... Как дела?"),
            vec!["Привет...", "Как дела?"]
        );
        // single \n is whitespace and triggers the boundary
        assert_eq!(
            split_by_sentences("Однажды.\nЯ вышел."),
            vec!["Однажды.", "Я вышел."]
        );
    }

    // #15 — UTF-8 paranoid: small max_chars, 4-byte emoji must not be split mid-byte.
    #[test]
    fn test_utf8_no_panic_on_boundary_split() {
        let text = "ёёё🎉ёёё";
        assert_eq!(text.chars().count(), 7);
        let chunks = chunk_text(text, 4);
        assert!(!chunks.is_empty());
        let has_emoji = chunks.iter().any(|c| c.contains('🎉'));
        assert!(
            has_emoji,
            "emoji must survive intact; got chunks: {:?}",
            chunks
        );
        for c in &chunks {
            assert!(c.chars().count() <= 4);
        }
    }

    // Bonus: max_chars == 0 returns empty Vec without panic.
    #[test]
    fn test_max_chars_zero_returns_empty_vec() {
        assert!(chunk_text("Hello.", 0).is_empty());
    }
}
