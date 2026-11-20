//! Select small, bounded excerpts and accept changes only at their targets.
use regex::Regex;
use std::{ops::Range, sync::LazyLock};

pub(super) const MAX_FRAGMENTS: usize = 120;
const MAX_FRAGMENT_CHARS: usize = 160;
const MAX_DISCLOSED_CHARS: usize = 8_000;

static TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\S+").unwrap());
static NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[-−+№$€₽£]?[0-9]+(?:[.,:/–-][0-9]+)*(?:[%$€₽£]|(?i:st|nd|rd|th|bn|mln|m|k))?$")
        .unwrap()
});
static PRIVATE_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[0-9]{7,}\b|\b[0-9]{1,3}(?:\.[0-9]{1,3}){3}\b|\+?[0-9][ \t-]*\(?[0-9]{3}\)?[ \t-]*[0-9]{3}[ \t-]*[0-9]{2}[ \t-]*[0-9]{2}\b").unwrap()
});

pub(super) struct Fragment {
    pub range: Range<usize>,
    targets: Vec<Range<usize>>,
}

struct Token {
    full: Range<usize>,
    core: Range<usize>,
    target: bool,
    blocked: bool,
    number: bool,
    ends_sentence: bool,
}

fn overlaps(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

fn word_target(word: &str, english: bool, follows_number: bool) -> bool {
    let length = word.chars().count();
    let abbreviation = (2..=7).contains(&length) && word.chars().all(char::is_uppercase);
    let latin = !english
        && word.chars().all(|c| c.is_ascii_alphabetic() || c == '-')
        && word.len() > 1
        && word.bytes().any(|b| b.is_ascii_alphabetic());
    abbreviation
        || latin
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
    let budget =
        (original_text.chars().filter(|c| !c.is_whitespace()).count() / 3).min(MAX_DISCLOSED_CHARS);
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
        let number = NUMBER.is_match(core);
        let follows_number = tokens.last().is_some_and(|previous| {
            previous.number
                && !previous.ends_sentence
                && !text[previous.full.end..token.start()].contains(['\n', '\r'])
        });
        tokens.push(Token {
            full: token.range(),
            core: start..start + core.len(),
            target: !blocked && (number || word_target(core, english, follows_number)),
            number: number || follows_number && word_target(core, english, true),
            blocked,
            ends_sentence: raw
                .trim_end_matches(['»', '"', '”', '’', '\'', ')', ']', '}'])
                .ends_with(['.', '!', '?', ';', '…']),
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
            });
        }
    }
    let mut remaining = budget;
    candidates
        .into_iter()
        .filter(|fragment| {
            let chars = text[fragment.range.clone()].chars().count();
            if chars > MAX_FRAGMENT_CHARS || chars > remaining {
                return false;
            }
            remaining -= chars;
            true
        })
        .take(MAX_FRAGMENTS)
        .collect()
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

    #[test]
    fn excerpts_stay_inside_sentences_and_keep_context() {
        let text = "Большой вводный абзац о результатах работы и о планах на далёкое будущее. Около 980 млн человек пришли. Следующее предложение тоже остаётся на компьютере.";
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
        let text = "Вводная часть довольно длинного текста без специальных выражений. Связь test9@example.org. Номер +7 (999) 123-45-67. Особая замена ABC. Рост 45% за год.";
        let keep = text.find("ABC").unwrap();
        let fragments = select(text, std::slice::from_ref(&(keep..keep + 3)), false, text);
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
    fn overlapping_context_is_sent_once_and_english_allows_latin_words() {
        let text = "There is substantial background on this project elsewhere in the report. About 10 cats and 20 dogs arrived. That is all the information available now.";
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
}
