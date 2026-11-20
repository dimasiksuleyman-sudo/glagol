//! Select small, bounded excerpts and accept changes only at their targets.
use regex::Regex;
use std::{ops::Range, sync::LazyLock};

pub(super) const MAX_FRAGMENTS: usize = 120;
const MAX_FRAGMENT_CHARS: usize = 160;
const MAX_DISCLOSED_CHARS: usize = 8_000;
/// One part in this many of the original text may be sent.
const DISCLOSED_SHARE: usize = 10;

static TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\S+").unwrap());
static NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[-−+№$€₽£]?[0-9]+(?:[.,:/–-][0-9]+)*(?:[%$€₽£]|(?i:st|nd|rd|th|bn|mln|m|k))?$")
        .unwrap()
});
static PRIVATE_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[0-9]{7,}\b|\b[0-9]{1,3}(?:\.[0-9]{1,3}){3}\b|\+?[0-9][ \t-]*\(?[0-9]{3}\)?[ \t-]*[0-9]{3}[ \t-]*[0-9]{2}[ \t-]*[0-9]{2}\b").unwrap()
});

/// Abbreviations written with a dot that do not end a sentence by themselves.
const DOTTED: &[&str] = &[
    "кв", "тыс", "млн", "млрд", "трлн", "руб", "долл", "г", "гг", "ст", "ул", "д", "т", "п", "см",
];

pub(super) struct Fragment {
    pub range: Range<usize>,
    targets: Vec<Range<usize>>,
    /// Contains a number whose case the local rules cannot tell.
    hard: bool,
}

struct Token {
    full: Range<usize>,
    core: Range<usize>,
    target: bool,
    blocked: bool,
    number: bool,
    hard: bool,
    ends_sentence: bool,
}

fn overlaps(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

fn word_target(word: &str, follows_number: bool) -> bool {
    // Names and other Latin words are content, not notation: they stay local
    // (the dictionary and the built-in rules handle them).
    let length = word.chars().count();
    let abbreviation = (2..=7).contains(&length) && word.chars().all(char::is_uppercase);
    abbreviation
        || follows_number
            && matches!(
                word.to_lowercase().as_str(),
                "тыс"
                    | "млн"
                    | "млрд"
                    | "трлн"
                    | "руб"
                    | "долл"
                    | "р"
                    | "км"
                    | "кг"
                    | "кв"
                    | "м"
                    | "км/ч"
                    | "г"
                    | "гг"
                    | "₽"
                    | "$"
                    | "%"
                    | "€"
                    | "£"
                    | "million"
                    | "billion"
                    | "trillion"
                    | "bn"
            )
}

pub(super) fn select(
    text: &str,
    protected: &[Range<usize>],
    english: bool,
    original_text: &str,
) -> Vec<Fragment> {
    // At most a tenth of the document (non-space characters) leaves the computer.
    let budget = (original_text.chars().filter(|c| !c.is_whitespace()).count() / DISCLOSED_SHARE)
        .min(MAX_DISCLOSED_CHARS);
    let mut blocked_ranges = protected.to_vec();
    blocked_ranges.extend(crate::text::preprocessor::contact_ranges(text));
    blocked_ranges.extend(PRIVATE_NUMBER.find_iter(text).map(|m| m.range()));
    blocked_ranges.sort_unstable_by_key(|r| r.start);
    let mut blocked_ranges = blocked_ranges.into_iter().peekable();
    let mut tokens: Vec<Token> = Vec::new();
    for token in TOKEN.find_iter(text) {
        let raw = token.as_str();
        let front = raw.trim_start_matches(['«', '"', '“', '‘', '\'', '(', '[', '{', ',', '—']);
        let core = front.trim_end_matches([
            '»', '"', '”', '’', '\'', ')', ']', '}', ',', '.', '!', '?', ';', ':',
        ]);
        let start = token.end() - front.len();
        while blocked_ranges
            .peek()
            .is_some_and(|r| r.end <= token.start())
        {
            blocked_ranges.next();
        }
        let blocked = core.parse::<std::net::IpAddr>().is_ok()
            || raw.contains(['@', '\\', '=', '_'])
            || (raw.contains('/') && !NUMBER.is_match(core) && core != "км/ч")
            || blocked_ranges
                .peek()
                .is_some_and(|r| overlaps(r, &token.range()));
        // «1,5 тыс.» is «полторы тысячи»: the rules read it better than models do.
        let one_and_a_half = !english && matches!(core, "1,5" | "1.5");
        let number = NUMBER.is_match(core) && !one_and_a_half;
        let hard = number
            && !english
            && !crate::text::normalize::ru::case_is_certain(
                &text[..token.start()],
                &text[token.end()..],
            );
        let follows_number = tokens.last().is_some_and(|previous| {
            previous.number
                && !previous.ends_sentence
                && !text[previous.full.end..token.start()].contains(['\n', '\r'])
        });
        tokens.push(Token {
            full: token.range(),
            core: start..start + core.len(),
            target: !blocked && (number || word_target(core, follows_number)),
            number: number || follows_number && word_target(core, true),
            hard,
            blocked,
            // «кв. м», «тыс. руб.» inside a sentence: an abbreviation's dot
            // followed by a lowercase word does not end the sentence.
            ends_sentence: raw
                .trim_end_matches(['»', '"', '”', '’', '\'', ')', ']', '}'])
                .ends_with(['.', '!', '?', ';', '…'])
                && !(raw.ends_with('.')
                    && DOTTED.contains(&core.to_lowercase().as_str())
                    && text[token.end()..]
                        .trim_start()
                        .chars()
                        .next()
                        .is_some_and(char::is_lowercase)),
        });
    }

    let neighboring = |left: &Token, right: &Token| {
        !left.blocked
            && !right.blocked
            && !left.ends_sentence
            && !text[left.full.end..right.full.start].contains(['\n', '\r'])
    };
    let mut candidates: Vec<Fragment> = Vec::new();
    for (i, token) in tokens.iter().enumerate().filter(|(_, t)| t.target) {
        let mut start = i;
        let mut end = i;
        if i > 0 && neighboring(&tokens[i - 1], token) {
            start -= 1;
        }
        while end < (i + 2).min(tokens.len() - 1) && neighboring(&tokens[end], &tokens[end + 1]) {
            end += 1;
        }
        let range = tokens[start].full.start..tokens[end].full.end;
        if let Some(previous) = candidates
            .last_mut()
            .filter(|previous| overlaps(&previous.range, &range))
        {
            previous.range.end = previous.range.end.max(range.end);
            previous.hard |= token.hard;
            let last = previous
                .targets
                .last_mut()
                .expect("a fragment has a target");
            if text[last.end..token.core.start]
                .chars()
                .all(char::is_whitespace)
            {
                last.end = token.core.end;
            } else {
                previous.targets.push(token.core.clone());
            }
        } else {
            candidates.push(Fragment {
                range,
                targets: vec![token.core.clone()],
                hard: token.hard,
            });
        }
    }
    // Hard numbers first, then the rest, while the budget lasts; the output
    // keeps text order.
    let mut remaining = budget;
    let mut chosen: Vec<Fragment> = Vec::new();
    for hard in [true, false] {
        for fragment in candidates.iter().filter(|f| f.hard == hard) {
            let chars = text[fragment.range.clone()].chars().count();
            if chosen.len() == MAX_FRAGMENTS || chars > MAX_FRAGMENT_CHARS || chars > remaining {
                continue;
            }
            remaining -= chars;
            chosen.push(Fragment {
                range: fragment.range.clone(),
                targets: fragment.targets.clone(),
                hard,
            });
        }
    }
    chosen.sort_unstable_by_key(|f| f.range.start);
    chosen
}

impl Fragment {
    /// Copy the unchanged context verbatim. Only the target spans may become words.
    pub fn validate(&self, text: &str, reply: &str, english: bool) -> Option<String> {
        let source = &text[self.range.clone()];
        let reply = reply.trim().replace('\u{301}', "");
        if reply.chars().count() > (source.chars().count() * 6).max(80) {
            return None;
        }
        let letters = if english {
            "[A-Za-z]"
        } else {
            r"[\p{Cyrillic}]"
        };
        let mut pattern = String::from(r"\A");
        let mut at = self.range.start;
        for target in &self.targets {
            pattern.push_str(&regex::escape(&text[at..target.start]));
            pattern.push_str(&format!("{letters}+(?:[ '’\u{2011}-]{letters}+)*"));
            at = target.end;
        }
        pattern.push_str(&regex::escape(&text[at..self.range.end]));
        pattern.push_str(r"\z");
        Regex::new(&pattern).ok()?.is_match(&reply).then_some(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Local-only filler so the excerpts stay within a tenth of the text.
    const FILLER_RU: &str = "Этот длинный абзац описывает общий контекст документа и никогда не покидает компьютер, потому что в нём нет ни чисел, ни сокращений, ни других особых обозначений. ";
    const FILLER_EN: &str = "This long paragraph describes the general context of the document and never leaves the computer because it has no numbers, abbreviations or other special notation. ";

    #[test]
    fn excerpts_stay_inside_sentences_and_keep_context() {
        let text = &format!("{FILLER_RU}Около 980 млн человек пришли. {FILLER_RU}");
        let fragments = select(text, &[], false, text);
        let [fragment] = fragments.as_slice() else {
            panic!("expected one fragment")
        };
        assert_eq!(
            &text[fragment.range.clone()],
            "Около 980 млн человек пришли."
        );
        assert_eq!(
            fragment
                .validate(
                    text,
                    "Около девятисо́т восьмидесяти миллионов человек пришли.",
                    false
                )
                .as_deref(),
            Some("Около девятисот восьмидесяти миллионов человек пришли.")
        );
        for invalid in [
            "Почти девятьсот миллионов человек пришли.",
            "Около 980 миллионов человек пришли.",
            "Около nine миллионов человек пришли.",
            "Около девятисот миллионов человек ушли.",
        ] {
            assert_eq!(fragment.validate(text, invalid, false), None);
        }
    }

    #[test]
    fn avoids_full_text_contacts_identifiers_and_dictionary_output() {
        assert!(select("Цена 20 рублей.", &[], false, "Цена 20 рублей.").is_empty());
        let text = &format!("{FILLER_RU}{FILLER_RU}Связь test9@example.org. Номер +7 (999) 123-45-67. Особая замена ABC. Рост 45% за год. Компания Anthropic.");
        let keep = text.find("ABC").unwrap();
        let fragments = select(text, std::slice::from_ref(&(keep..keep + 3)), false, text);
        // Latin names («Anthropic») are not sent either.
        assert_eq!(fragments.len(), 1);
        assert_eq!(&text[fragments[0].range.clone()], "Рост 45% за год.");
    }

    #[test]
    fn whitespace_and_dictionary_expansion_cannot_increase_the_budget() {
        let padded = format!("{}Цена 20 рублей.", " ".repeat(100));
        assert!(select(&padded, &[], false, &padded).is_empty());

        let original = "До. Цена 20 рублей.";
        let expanded = format!("{}. Цена 20 рублей.", "Длинная замена ".repeat(10));
        assert!(select(&expanded, &[], false, original).is_empty());
    }

    #[test]
    fn overlapping_context_is_sent_once() {
        let text = &format!(
            "{FILLER_EN}{FILLER_EN}About 10 cats and 20 dogs arrived. {FILLER_EN}{FILLER_EN}"
        );
        let fragments = select(text, &[], true, text);
        assert_eq!(fragments.len(), 1);
        let fragment = &fragments[0];
        assert_eq!(
            &text[fragment.range.clone()],
            "About 10 cats and 20 dogs arrived."
        );
        assert!(fragment
            .validate(text, "About ten cats and twenty dogs arrived.", true)
            .is_some());
        assert!(fragment
            .validate(text, "About ten cats and twenty birds arrived.", true)
            .is_none());
    }

    #[test]
    fn hard_numbers_go_first_and_one_and_a_half_stays_local() {
        let text = &format!(
            "{FILLER_RU}Выручка за 2026 г. составила 4,8 млрд руб. и ещё 730 млн руб. {FILLER_RU}Акционеры получат 18 руб. на акцию. Из 1,5 тыс. заявок одобрена треть."
        );
        let sent: Vec<&str> = select(text, &[], false, text)
            .iter()
            .map(|f| &text[f.range.clone()])
            .collect();
        // The uncertain «получат 18 руб.» is chosen before the easier ones.
        assert!(sent.iter().any(|f| f.contains("18 руб.")), "{sent:?}");
        assert!(!sent.iter().any(|f| f.contains("1,5")), "{sent:?}");
        // An abbreviation's dot does not cut «кв. м» in half.
        let area = &format!("{FILLER_RU}{FILLER_RU}Склад площадью 300 кв. м в центре. {FILLER_RU}");
        let sent: Vec<&str> = select(area, &[], false, area)
            .iter()
            .map(|f| &area[f.range.clone()])
            .collect();
        assert!(sent.iter().any(|f| f.contains("300 кв. м")), "{sent:?}");
        let starts: Vec<usize> = select(text, &[], false, text)
            .iter()
            .map(|f| f.range.start)
            .collect();
        assert!(starts.windows(2).all(|w| w[0] < w[1]));
    }
}
