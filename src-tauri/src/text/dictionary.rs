//! The user's pronunciation dictionary (tts-normalization N2): «как написано →
//! как читать», applied before the built-in normalization so the user's
//! choice always wins. Also CSV import/export.

use regex::RegexBuilder;
use serde::{Deserialize, Serialize};

/// Longest accepted pattern and replacement.
pub const MAX_PATTERN: usize = 100;
pub const MAX_REPLACEMENT: usize = 300;
/// Most entries one dictionary may hold.
pub const MAX_ENTRIES: usize = 5000;

/// One dictionary entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Database id; `None` for a new entry.
    pub id: Option<i64>,
    /// How it is written.
    pub pattern: String,
    /// How to read it; «+» before a vowel marks stress («з+амок»).
    pub replacement: String,
    /// `ru`, `en` or `any`.
    pub language: String,
    /// Match whole words only.
    pub whole_word: bool,
    /// Match letter case exactly.
    pub case_sensitive: bool,
    /// Used during synthesis.
    pub enabled: bool,
}

/// Check an entry before saving; returns it trimmed.
pub fn validate(mut entry: Entry) -> Result<Entry, String> {
    entry.pattern = entry.pattern.trim().to_string();
    entry.replacement = entry.replacement.trim().to_string();
    if entry.pattern.is_empty() {
        return Err("Укажите, как слово написано.".into());
    }
    if entry.replacement.is_empty() {
        return Err("Укажите, как слово читать.".into());
    }
    if entry.pattern.chars().count() > MAX_PATTERN
        || entry.replacement.chars().count() > MAX_REPLACEMENT
    {
        return Err("Слишком длинная запись словаря.".into());
    }
    if !matches!(entry.language.as_str(), "ru" | "en" | "any") {
        return Err("Недопустимый язык записи словаря.".into());
    }
    Ok(entry)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// One capture group for `entry`, with its own case and word-boundary rules.
fn pattern_source(entry: &Entry) -> String {
    let escaped = regex::escape(&entry.pattern);
    let starts_word = entry.pattern.chars().next().is_some_and(is_word_char);
    let ends_word = entry.pattern.chars().last().is_some_and(is_word_char);
    let body = if entry.whole_word {
        format!(
            "{}{escaped}{}",
            if starts_word { r"\b" } else { "" },
            if ends_word { r"\b" } else { "" }
        )
    } else {
        escaped
    };
    if entry.case_sensitive {
        format!("({body})")
    } else {
        format!("((?i:{body}))")
    }
}

/// Apply the enabled entries for `language` (`ru`/`en`) to `text` in a single
/// pass: at each position the longest matching pattern wins («Нью-Йорк Таймс»
/// over «Нью-Йорк»), and replaced text is never matched again.
pub fn apply(text: &str, entries: &[Entry], language: &str) -> String {
    apply_tracking(text, entries, language, |_| {})
}

/// Apply the dictionary and record output spans to keep local during normalization.
pub fn apply_with_ranges(
    text: &str,
    entries: &[Entry],
    language: &str,
) -> (String, Vec<std::ops::Range<usize>>) {
    let mut protected = Vec::new();
    let output = apply_tracking(text, entries, language, |range| protected.push(range));
    (output, protected)
}

fn apply_tracking(
    text: &str,
    entries: &[Entry],
    language: &str,
    mut track: impl FnMut(std::ops::Range<usize>),
) -> String {
    let mut active: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.enabled && (e.language == language || e.language == "any"))
        .collect();
    if active.is_empty() {
        return text.to_string();
    }
    active.sort_by_key(|e| std::cmp::Reverse(e.pattern.chars().count()));
    let source: Vec<String> = active.iter().map(|e| pattern_source(e)).collect();
    let Ok(re) = RegexBuilder::new(&source.join("|"))
        .size_limit(64 * 1024 * 1024)
        .build()
    else {
        tracing::warn!("pronunciation dictionary could not be compiled; skipped");
        return text.to_string();
    };
    let mut output = String::with_capacity(text.len());
    let mut end = 0;
    for captures in re.captures_iter(text) {
        let matched = captures.get(0).expect("a capture has a full match");
        output.push_str(&text[end..matched.start()]);
        let start = output.len();
        let replacement = (1..captures.len())
            .find(|&i| captures.get(i).is_some())
            .map_or(matched.as_str(), |i| active[i - 1].replacement.as_str());
        output.push_str(replacement);
        track(start..output.len());
        end = matched.end();
    }
    output.push_str(&text[end..]);
    output
}

/// Remove stress marks («+» right before a vowel) for engines that do not
/// understand them; other plus signs («C++», «2+2») stay.
pub fn strip_stress(text: &str) -> String {
    let vowel = |c: char| "аеёиоуыэюяАЕЁИОУЫЭЮЯaeiouyAEIOUY".contains(c);
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '+' && chars.peek().is_some_and(|n| vowel(*n)) {
            continue;
        }
        out.push(c);
    }
    out
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Export entries as CSV: `pattern,replacement,language,whole_word,case_sensitive,enabled`.
pub fn to_csv(entries: &[Entry]) -> String {
    let mut out = String::from("pattern,replacement,language,whole_word,case_sensitive,enabled\n");
    for e in entries {
        out.push_str(&format!(
            "{},{},{},{},{},{}\n",
            csv_field(&e.pattern),
            csv_field(&e.replacement),
            e.language,
            e.whole_word as u8,
            e.case_sensitive as u8,
            e.enabled as u8
        ));
    }
    out
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', true) => quoted = false,
            ('"', false) if field.is_empty() => quoted = true,
            (',' | ';', false) => fields.push(std::mem::take(&mut field)),
            (c, _) => field.push(c),
        }
    }
    fields.push(field);
    fields
}

/// Parse CSV exported by [`to_csv`] (or a two-column «написано,читать» table,
/// comma- or semicolon-separated). Invalid rows are skipped and counted.
pub fn from_csv(text: &str) -> (Vec<Entry>, usize) {
    let mut entries = Vec::new();
    let mut skipped = 0;
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_start_matches('\u{feff}');
        if line.trim().is_empty() || (i == 0 && line.starts_with("pattern")) {
            continue;
        }
        let f = parse_csv_line(line);
        let flag = |i: usize, default: bool| f.get(i).map_or(default, |v| v.trim() != "0");
        let entry = Entry {
            id: None,
            pattern: f.first().cloned().unwrap_or_default(),
            replacement: f.get(1).cloned().unwrap_or_default(),
            language: f
                .get(2)
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .unwrap_or_else(|| "any".into()),
            whole_word: flag(3, true),
            case_sensitive: flag(4, false),
            enabled: flag(5, true),
        };
        match validate(entry) {
            Ok(e) if entries.len() < MAX_ENTRIES => entries.push(e),
            _ => skipped += 1,
        }
    }
    (entries, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn whole_words_case_insensitive_longest_first() {
        let entries = vec![
            entry("Нью-Йорк", "Нью Йорк"),
            entry("Нью-Йорк Таймс", "Нью-Йорк Таймз"),
            entry("Anthropic", "Энтропик"),
            entry("ИИ", "искусственный интеллект"),
        ];
        assert_eq!(
            apply(
                "Газета Нью-Йорк Таймс пишет: anthropic и ИИ.",
                &entries,
                "ru"
            ),
            "Газета Нью-Йорк Таймз пишет: Энтропик и искусственный интеллект."
        );
        // «ИИ» inside a word is left alone.
        assert_eq!(apply("ИИСУС", &entries, "ru"), "ИИСУС");
    }

    #[test]
    fn language_case_and_enabled_flags() {
        let mut ru_only = entry("Claude", "Клод");
        ru_only.language = "ru".into();
        let mut exact = entry("IT", "ай-ти");
        exact.case_sensitive = true;
        let mut off = entry("тест", "проверка");
        off.enabled = false;
        let mut part = entry("кг", "килограмм");
        part.whole_word = false;
        let entries = vec![ru_only, exact, off, part];
        assert_eq!(
            apply("Claude it IT тест 5кг", &entries, "ru"),
            "Клод it ай-ти тест 5килограмм"
        );
        assert_eq!(apply("Claude", &entries, "en"), "Claude");
    }

    #[test]
    fn replacement_is_literal_and_stress_can_be_stripped() {
        let entries = vec![entry("замок", "з+амок $1")];
        let out = apply("старый замок", &entries, "ru");
        assert_eq!(out, "старый з+амок $1");
        assert_eq!(strip_stress(&out), "старый замок $1");
        assert_eq!(strip_stress("C++ и 2+2"), "C++ и 2+2");
    }

    #[test]
    fn csv_round_trip_and_simple_tables() {
        let mut quoted = entry("Smith, Inc.", "Смит \"Инк\"");
        quoted.language = "en".into();
        let entries = vec![entry("IPO", "ай-пи-о"), quoted];
        let (back, skipped) = from_csv(&to_csv(&entries));
        assert_eq!(skipped, 0);
        assert_eq!(back, entries);
        let (simple, skipped) = from_csv("\u{feff}ВВП;вэ-вэ-пэ\nпусто;\n;нет\nЦБ;цэ-бэ\n");
        assert_eq!(simple.len(), 2);
        assert_eq!(skipped, 2);
        assert_eq!(simple[0].language, "any");
        assert!(simple[1].whole_word && simple[1].enabled);
    }

    #[test]
    fn validation() {
        assert!(validate(entry(" ", "x")).is_err());
        assert!(validate(entry("x", " ")).is_err());
        let mut bad = entry("x", "y");
        bad.language = "de".into();
        assert!(validate(bad).is_err());
        assert_eq!(validate(entry(" a ", " b ")).unwrap().pattern, "a");
    }
}
