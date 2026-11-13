//! Removal of hesitation sounds from a dictated transcript.
//!
//! People thinking aloud say «э-э», «эм-м», «м-м», «хм» or "uh", "um", "hmm";
//! recognizers often write them down. This filter drops such *sounds* — words
//! made only of hesitation letters — and repairs the punctuation and
//! capitalization around them. Real words that are also used as fillers
//! («ну», «вот», «типа», "like") are kept: without understanding the sentence
//! they cannot be told apart from their ordinary use. Single «м», «а», «у»
//! are kept too (a unit, a conjunction, a preposition).
//!
//! The function is pure and runs on the transcript in memory only; nothing is
//! logged.

/// Characters that end a sentence.
const TERMINAL: &[char] = &['.', '!', '?', '…'];

/// Whether `word` (lowercase, hyphens normalized, no surrounding punctuation)
/// is a hesitation sound.
fn is_filler(word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    if word.contains('-') {
        // Russian hyphenated sounds: «э-э», «м-м», «эм-м», «а-а», «ы-ы», «хм-м».
        // English hyphenated forms ("uh-huh", "mm-hmm", "uh-oh") mean yes/no.
        let parts: Vec<&str> = word.split('-').collect();
        if parts.iter().any(|p| p.is_empty()) {
            return false;
        }
        let letters: Vec<char> = word.chars().filter(|c| *c != '-').collect();
        let only = |set: &[char]| letters.iter().all(|c| set.contains(c));
        return only(&['э', 'м'])
            || only(&['а'])
            || only(&['ы'])
            || (letters[0] == 'х' && only(&['х', 'м']));
    }
    let chars: Vec<char> = word.chars().collect();
    // `first+ second+` with at least `min_first` leading characters.
    let runs = |first: char, second: Option<char>, min_first: usize| -> bool {
        let lead = chars.iter().take_while(|c| **c == first).count();
        if lead < min_first {
            return false;
        }
        match second {
            None => lead == chars.len(),
            Some(second) => {
                let rest = &chars[lead..];
                !rest.is_empty() && rest.iter().all(|c| *c == second)
            }
        }
    };
    // Russian: э, ээ; эм, эмм; мм, ммм; хм, хмм; аа; ыы.
    runs('э', None, 1)
        || runs('э', Some('м'), 1)
        || runs('м', None, 2)
        || runs('х', Some('м'), 1)
        || runs('а', None, 2)
        || runs('ы', None, 2)
        // English: uh, uhh; um, umm; hm, hmm; mm, mmm.
        || runs('u', Some('h'), 1)
        || runs('u', Some('m'), 1)
        || runs('h', Some('m'), 1)
        || runs('m', None, 2)
        // English: er, erm, ermm — but not "err", which is a verb.
        || matches!(word, "er" | "erm" | "ermm" | "errm")
}

/// The word inside `token` without surrounding punctuation, normalized for
/// [`is_filler`], plus the token's trailing punctuation.
fn split_token(token: &str) -> (String, &str) {
    let core = token.trim_matches(|c: char| !c.is_alphanumeric());
    let core_end = token.rfind(core).map(|i| i + core.len()).unwrap_or(0);
    let trailing = if core.is_empty() {
        ""
    } else {
        &token[core_end..]
    };
    let word = core
        .chars()
        .map(|c| match c {
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' => '-',
            other => other,
        })
        .flat_map(char::to_lowercase)
        .collect();
    (word, trailing)
}

/// Upper-case the first letter of `token`, keeping any leading punctuation.
fn capitalize(token: &str) -> String {
    let mut out = String::with_capacity(token.len());
    let mut done = false;
    for c in token.chars() {
        if !done && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            done = true;
        } else {
            out.push(c);
        }
    }
    out
}

/// Remove hesitation sounds from one line of text.
fn clean_line(line: &str) -> String {
    let mut kept: Vec<String> = Vec::new();
    let mut capitalize_next = false;
    let mut removed_at_end = false;
    for token in line.split_whitespace() {
        let (word, trailing) = split_token(token);
        if is_filler(&word) {
            removed_at_end = true;
            // A capitalized sound started a sentence: the next word takes over.
            if token
                .chars()
                .find(|c| c.is_alphabetic())
                .is_some_and(char::is_uppercase)
            {
                capitalize_next = true;
            }
            // Keep a sentence end the sound carried: «хорошо, эм.» → «хорошо.»
            let terminal: String = trailing.chars().filter(|c| TERMINAL.contains(c)).collect();
            if let Some(previous) = kept.last_mut() {
                if !terminal.is_empty() && !previous.ends_with(TERMINAL) {
                    let trimmed_len = previous.trim_end_matches([',', ';', ':']).len();
                    previous.truncate(trimmed_len);
                    previous.push_str(&terminal);
                }
            }
            continue;
        }
        removed_at_end = false;
        let is_punctuation = !token.chars().any(char::is_alphanumeric);
        // «Да — эм — нет» must not become «Да — — нет».
        if is_punctuation
            && kept
                .last()
                .is_some_and(|p| !p.chars().any(char::is_alphanumeric))
        {
            continue;
        }
        if capitalize_next && !is_punctuation {
            kept.push(capitalize(token));
            capitalize_next = false;
        } else {
            kept.push(token.to_string());
        }
    }
    // «Привет, эм» → «Привет»: a comma left before removed sounds at the end.
    if removed_at_end {
        if let Some(last) = kept.last_mut() {
            let trimmed_len = last.trim_end_matches([',', ';', ':']).len();
            last.truncate(trimmed_len);
        }
        while kept
            .last()
            .is_some_and(|p| !p.chars().any(char::is_alphanumeric))
        {
            kept.pop();
        }
    }
    kept.join(" ")
}

/// Remove hesitation sounds («э-э», «эм», «м-м», «хм», "uh", "um", "hmm"…)
/// from a transcript and repair the punctuation around them. Returns the text
/// unchanged when it has none; an empty string when it had nothing else.
pub fn remove_fillers(text: &str) -> String {
    text.lines()
        .map(clean_line)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_russian_hesitation_sounds() {
        assert_eq!(
            remove_fillers("Эм-м, я думаю, что это так."),
            "Я думаю, что это так."
        );
        assert_eq!(
            remove_fillers("Я думаю, э-э, что это так."),
            "Я думаю, что это так."
        );
        assert_eq!(remove_fillers("Надо ммм купить хлеб"), "Надо купить хлеб");
        assert_eq!(remove_fillers("Хм, интересно."), "Интересно.");
        assert_eq!(remove_fillers("Это хорошо, эм."), "Это хорошо.");
        assert_eq!(remove_fillers("Привет, эм"), "Привет");
        assert_eq!(remove_fillers("Да. Ээ, давай завтра."), "Да. Давай завтра.");
        assert_eq!(remove_fillers("А-а, понятно"), "Понятно");
        assert_eq!(remove_fillers("Да — эм — нет"), "Да — нет");
    }

    #[test]
    fn removes_english_hesitation_sounds() {
        assert_eq!(remove_fillers("Um, I think so."), "I think so.");
        assert_eq!(remove_fillers("It is, uh, fine"), "It is, fine");
        assert_eq!(
            remove_fillers("Hmm. Let me see, er, tomorrow."),
            "Let me see, tomorrow."
        );
        assert_eq!(remove_fillers("so umm we go"), "so we go");
    }

    #[test]
    fn keeps_real_words_and_meaningful_sounds() {
        for text in [
            "Ну и что, вот документ типа данных.",
            "Длина 5 м, а у нас 3.",
            "Мэм, это мем.",
            "Uh-huh, mm-hmm, uh-oh.",
            "To err is human.",
            "Umbrella, hummus, mmr.",
            "Эмоции и эмблема.",
        ] {
            assert_eq!(remove_fillers(text), text, "{text}");
        }
    }

    #[test]
    fn only_sounds_leave_nothing() {
        assert_eq!(remove_fillers("Эм-м... э-э."), "");
        assert_eq!(remove_fillers("  "), "");
    }

    #[test]
    fn lowercase_transcripts_stay_lowercase() {
        assert_eq!(remove_fillers("эм привет как дела"), "привет как дела");
    }

    #[test]
    fn recognizes_sound_shapes() {
        for w in [
            "э", "эээ", "эм", "ээмм", "мм", "м-м", "э-э-э", "эм-м", "хм", "хм-м", "аа", "ыы", "uh",
            "umm", "hmm", "mm", "er", "erm",
        ] {
            assert!(is_filler(w), "{w}");
        }
        for w in [
            "м",
            "а",
            "у",
            "мэм",
            "эмоции",
            "err",
            "uh-huh",
            "mm-hmm",
            "-",
            "э-",
            "ах",
            "ок",
        ] {
            assert!(!is_filler(w), "{w}");
        }
    }
}
