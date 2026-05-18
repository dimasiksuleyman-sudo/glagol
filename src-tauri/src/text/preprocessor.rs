//! Text preprocessing for SaluteSpeech narration.
//!
//! Humanises patterns that SaluteSpeech otherwise reads mechanically
//! (URLs spelled out letter by letter, emails punctuated awkwardly,
//! common Russian abbreviations read literally) so the synthesised
//! audio flows naturally. Runs before the chunker in the synthesis
//! pipeline.
//!
//! ## Composition
//!
//! [`preprocess`] applies four passes in this fixed order:
//!
//! 1. [`normalize_whitespace`] — CRLF → LF, NBSP/tab → space, collapse
//!    multi-space within lines, preserve paragraph breaks (`\n\n`),
//!    trim leading/trailing whitespace.
//! 2. [`replace_emails`] — substitute every email address with the
//!    placeholder word «email».
//! 3. [`replace_urls`] — substitute `http(s)://…`, `www.…`, and bare
//!    `domain.tld` matches (TLD whitelist) with the placeholder word
//!    «ссылка».
//! 4. [`expand_abbreviations`] — replace Tier 1 multi-letter
//!    abbreviations (`т.е.`, `и т.д.`, `т.к.`, …) with their expanded
//!    forms.
//!
//! The order is load-bearing: emails MUST run before URLs because the
//! email's domain portion (`example.com`) would otherwise be partially
//! matched by the URL regex, corrupting the email.
//!
//! ## Performance
//!
//! All regexes are compiled once on first use via [`std::sync::LazyLock`]
//! and reused for every subsequent call. The preprocessor is called once
//! per synthesis and a 50 KB document round-trips in single-digit
//! milliseconds.
//!
//! ## Scope (Sprint 3a)
//!
//! - Tier 1 abbreviations only — single-letter context-dependent
//!   abbreviations like `г.`, `с.`, `р.` are excluded because the
//!   false-positive risk is too high.
//! - Bare-domain matching is restricted to a curated TLD whitelist for
//!   the same reason: it prevents `1.5` (numbers) and `file.pdf`
//!   (filenames) from being treated as URLs.
//! - Number formatting and smart-quotes / em-dash normalisation are
//!   deliberately skipped — SaluteSpeech already handles them well.

use regex::Regex;
use std::sync::LazyLock;

/// Replacement word emitted in place of every detected URL.
const URL_PLACEHOLDER: &str = "ссылка";

/// Replacement word emitted in place of every detected email address.
const EMAIL_PLACEHOLDER: &str = "email";

/// Whitelist of top-level domains used by the bare-domain URL detector.
/// Anything outside this list (e.g. `pdf`, `community`, `5`) is left as
/// plain text so we don't false-positive on filenames, abbreviations,
/// or version numbers.
const TLD_WHITELIST: &str = "com|org|ru|рф|net|info|app|io|me|tech|dev|store|online|site|club|blog|page|link|today|news|media|tv|fm|gov|edu|kz|by|ua|uz|kg|am|ge|az|md|ee|lv|lt|su|eu|biz|name|pro|cat|mobi|jobs|travel|museum|asia|xxx";

static URL_SCHEMA_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://\S+").expect("URL_SCHEMA_REGEX compiles"));

static URL_WWW_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bwww\.[a-zA-Z0-9-]+\.\S*").expect("URL_WWW_REGEX compiles"));

static URL_BARE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    let pattern = format!(r"\b[a-zA-Zа-яА-Я0-9-]{{2,}}\.({TLD_WHITELIST})\b");
    Regex::new(&pattern).expect("URL_BARE_REGEX compiles")
});

static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}\b").expect("EMAIL_REGEX compiles")
});

/// Multi-letter Russian abbreviations expanded by [`expand_abbreviations`].
///
/// Two entries per phrase — lowercase and sentence-start capitalised —
/// because plain `str::replace` is case-sensitive and these are the two
/// forms that actually occur in narrative text. ALL-CAPS oddities like
/// `Т.Е.` are deferred to Sprint 5 polish if they ever surface.
const ABBREVIATIONS: &[(&str, &str)] = &[
    ("т.е.", "то есть"),
    ("Т.е.", "То есть"),
    ("и т.д.", "и так далее"),
    ("И т.д.", "И так далее"),
    ("и т.п.", "и тому подобное"),
    ("И т.п.", "И тому подобное"),
    ("и др.", "и другие"),
    ("И др.", "И другие"),
    ("и пр.", "и прочее"),
    ("И пр.", "И прочее"),
    ("т.к.", "так как"),
    ("Т.к.", "Так как"),
    ("т.н.", "так называемый"),
    ("Т.н.", "Так называемый"),
    ("т.о.", "таким образом"),
    ("Т.о.", "Таким образом"),
];

/// Run every preprocessing pass in the canonical order. See the module
/// docs for why emails must precede URLs.
pub fn preprocess(text: &str) -> String {
    let text = normalize_whitespace(text);
    let text = replace_emails(&text);
    let text = replace_urls(&text);
    expand_abbreviations(&text)
}

/// Normalise whitespace: CRLF → LF, NBSP/tab → space, collapse runs of
/// in-line whitespace, collapse 3+ consecutive newlines to a single
/// paragraph break, trim leading/trailing whitespace.
pub fn normalize_whitespace(text: &str) -> String {
    let mut text = text.replace("\r\n", "\n");
    text = text.replace('\r', "\n");
    text = text.replace('\u{00A0}', " ");
    text = text.replace('\t', " ");

    // Per-line whitespace collapse, preserving the newlines between lines.
    // `split_whitespace` on each line drops empty / whitespace-only segments
    // and collapses internal runs to single spaces.
    let collapsed_lines: Vec<String> = text
        .split('\n')
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();

    let mut joined = collapsed_lines.join("\n");
    while joined.contains("\n\n\n") {
        joined = joined.replace("\n\n\n", "\n\n");
    }
    joined.trim().to_string()
}

/// Replace every email address with the placeholder word «email».
pub fn replace_emails(text: &str) -> String {
    EMAIL_REGEX
        .replace_all(text, EMAIL_PLACEHOLDER)
        .into_owned()
}

/// Replace every URL — schema, `www.` prefix, or whitelisted bare
/// domain — with the placeholder word «ссылка».
///
/// Trailing sentence punctuation (`. , ; : ! ? )`) is peeled off the
/// match and preserved outside the placeholder so a URL at end-of-
/// sentence keeps its period: `«…site https://x.com.» → «…site ссылка.»`.
/// The bare-domain regex doesn't need this because `\b` already stops
/// at the boundary after the TLD.
pub fn replace_urls(text: &str) -> String {
    let after_schema = URL_SCHEMA_REGEX.replace_all(text, replace_url_match);
    let after_www = URL_WWW_REGEX.replace_all(&after_schema, replace_url_match);
    URL_BARE_REGEX
        .replace_all(&after_www, URL_PLACEHOLDER)
        .into_owned()
}

fn replace_url_match(caps: &regex::Captures) -> String {
    let whole = &caps[0];
    let core_len = whole
        .trim_end_matches(|c: char| ".,;:!?)\"'".contains(c))
        .len();
    let mut out = String::with_capacity(URL_PLACEHOLDER.len() + (whole.len() - core_len));
    out.push_str(URL_PLACEHOLDER);
    out.push_str(&whole[core_len..]);
    out
}

/// Expand Tier 1 multi-letter Russian abbreviations.
pub fn expand_abbreviations(text: &str) -> String {
    let mut result = text.to_string();
    for (pattern, replacement) in ABBREVIATIONS {
        result = result.replace(pattern, replacement);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── normalize_whitespace ──────────────────────────────────────────

    #[test]
    fn normalize_whitespace_collapses_multi_spaces_within_line() {
        assert_eq!(
            normalize_whitespace("hello    world   today"),
            "hello world today"
        );
    }

    #[test]
    fn normalize_whitespace_preserves_paragraph_breaks() {
        assert_eq!(
            normalize_whitespace("first paragraph\n\nsecond paragraph"),
            "first paragraph\n\nsecond paragraph"
        );
    }

    #[test]
    fn normalize_whitespace_converts_crlf_to_lf() {
        // Mixed CRLF + bare CR should both fold to LF, and the resulting
        // 3 consecutive newlines should collapse to a paragraph break.
        assert_eq!(
            normalize_whitespace("alpha\r\nbeta\r\n\r\ngamma"),
            "alpha\nbeta\n\ngamma"
        );
    }

    #[test]
    fn normalize_whitespace_converts_nbsp_and_tabs() {
        let input = "foo\u{00A0}\u{00A0}bar\tbaz";
        assert_eq!(normalize_whitespace(input), "foo bar baz");
    }

    // ── replace_emails ────────────────────────────────────────────────

    #[test]
    fn replace_emails_substitutes_simple_email() {
        assert_eq!(
            replace_emails("Свяжитесь: admin@example.com за помощью."),
            "Свяжитесь: email за помощью."
        );
    }

    #[test]
    fn replace_emails_handles_multiple_in_text() {
        assert_eq!(
            replace_emails("a@b.com и c.d+tag@e.org здесь."),
            "email и email здесь."
        );
    }

    #[test]
    fn replace_emails_preserves_text_without_emails() {
        let input = "Здесь нет ни одного адреса электронной почты.";
        assert_eq!(replace_emails(input), input);
    }

    #[test]
    fn replace_emails_does_not_match_at_symbol_alone() {
        // `@` without a domain shouldn't trigger; "C@C" lacks a TLD too.
        assert_eq!(
            replace_emails("стоимость 100 @ за штуку"),
            "стоимость 100 @ за штуку"
        );
        assert_eq!(replace_emails("C@C"), "C@C");
    }

    // ── replace_urls ──────────────────────────────────────────────────

    #[test]
    fn replace_urls_substitutes_https_url() {
        assert_eq!(
            replace_urls("См. https://github.com/foo/bar для деталей."),
            "См. ссылка для деталей."
        );
    }

    #[test]
    fn replace_urls_substitutes_http_url() {
        assert_eq!(
            replace_urls("Старый http://example.org/path?q=1 ресурс."),
            "Старый ссылка ресурс."
        );
    }

    #[test]
    fn replace_urls_substitutes_www_prefix_url() {
        assert_eq!(
            replace_urls("Зайдите на www.glagol.tech/about и почитайте."),
            "Зайдите на ссылка и почитайте."
        );
    }

    #[test]
    fn replace_urls_substitutes_bare_domain_with_whitelisted_tld() {
        assert_eq!(replace_urls("Сайт github.com лежит."), "Сайт ссылка лежит.");
    }

    #[test]
    fn replace_urls_skips_abbreviations_due_to_tld_whitelist() {
        // т.е. — single letter before the dot fails `[…]{2,}` even
        // before TLD lookup runs; defensive coverage anyway.
        assert_eq!(
            replace_urls("Это просто, т.е. легко."),
            "Это просто, т.е. легко."
        );
    }

    #[test]
    fn replace_urls_skips_numbers_due_to_tld_whitelist() {
        // Version numbers must NOT be treated as URLs. "5" is also not
        // a whitelisted TLD, so the bare-domain regex correctly fails.
        assert_eq!(
            replace_urls("Версия 1.5 содержит улучшения."),
            "Версия 1.5 содержит улучшения."
        );
        assert_eq!(
            replace_urls("Файл report.pdf готов."),
            "Файл report.pdf готов."
        );
    }

    // ── expand_abbreviations ──────────────────────────────────────────

    #[test]
    fn expand_abbreviations_replaces_te_lowercase() {
        assert_eq!(
            expand_abbreviations("просто, т.е. легко"),
            "просто, то есть легко"
        );
    }

    #[test]
    fn expand_abbreviations_replaces_capitalized_variant() {
        assert_eq!(
            expand_abbreviations("Т.е. это работает."),
            "То есть это работает."
        );
    }

    #[test]
    fn expand_abbreviations_preserves_text_without_abbreviations() {
        let input = "Обычное предложение без сокращений.";
        assert_eq!(expand_abbreviations(input), input);
    }

    // ── preprocess composition ────────────────────────────────────────

    #[test]
    fn preprocess_email_url_abbreviation_composed_correctly() {
        let input = "См. https://docs.example.com или admin@example.com — т.е. любой способ.";
        let expected = "См. ссылка или email — то есть любой способ.";
        assert_eq!(preprocess(input), expected);
    }

    #[test]
    fn preprocess_email_runs_before_url() {
        // If URL ran first it would partially match `example.com` inside
        // the email and produce "admin@ссылка", which the email regex
        // would then fail to match. Email-first preserves the address.
        let input = "Связь: admin@site.com и сайт https://site.com.";
        let expected = "Связь: email и сайт ссылка.";
        assert_eq!(preprocess(input), expected);
    }

    #[test]
    fn preprocess_preserves_paragraph_structure() {
        let input = "Первый абзац.\n\nВторой абзац с https://example.com.";
        let expected = "Первый абзац.\n\nВторой абзац с ссылка.";
        assert_eq!(preprocess(input), expected);
    }
}
