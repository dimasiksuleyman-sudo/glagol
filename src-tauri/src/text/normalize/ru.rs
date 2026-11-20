//! Russian text normalization for speech: numbers, money, percentages,
//! dates, years, centuries, time and common abbreviations become words in the
//! grammatical case the context asks for. Pure and offline; anything it cannot
//! read safely is left for the speech engine.

use std::sync::LazyLock;

use regex::{Captures, Regex};

use super::ru_numbers::{
    agree, cardinal, decimal, one_and_a_half, ordinal, ordinal_plural, Case, Gender, Noun, BILLION,
    DOLLAR, EURO, KILOGRAM, KILOMETRE, MILLION, PERCENT, RUBLE, THOUSAND, TRILLION, YEAR,
};

const MONTHS_GEN: [&str; 12] = [
    "января",
    "февраля",
    "марта",
    "апреля",
    "мая",
    "июня",
    "июля",
    "августа",
    "сентября",
    "октября",
    "ноября",
    "декабря",
];

/// Abbreviations read letter by letter or by an established reading.
const ABBREVIATIONS: &[(&str, &str)] = &[
    ("IPO", "ай-пи-о"),
    ("CEO", "си-и-о"),
    ("AI", "эй-ай"),
    ("IT", "ай-ти"),
    ("USA", "ю-эс-эй"),
    ("USB", "ю-эс-би"),
    ("PDF", "пэ-дэ-эф"),
    ("GPU", "джи-пи-ю"),
    ("CPU", "си-пи-ю"),
    ("SpaceX", "спейс-икс"),
    ("OpenAI", "оупен-эй-ай"),
    ("США", "сэ-шэ-а"),
    ("РФ", "эр-эф"),
    ("ВВП", "вэ-вэ-пэ"),
    ("ЦБ", "цэ-бэ"),
    ("МВФ", "эм-вэ-эф"),
    ("ИИ", "и-и"),
];

/// The last word before `before`, inside the current sentence, lowercased.
fn previous_word(before: &str) -> String {
    let sentence = before
        .rfind(['.', '!', '?', '\n', ';', ':'])
        .map_or(before, |i| &before[i + 1..]);
    sentence
        .split(|c: char| !c.is_alphabetic() && c != '-')
        .rfind(|w| !w.is_empty())
        .unwrap_or("")
        .to_lowercase()
}

/// The first word after `after`, lowercased.
fn next_word(after: &str) -> String {
    after
        .trim_start_matches(|c: char| c.is_whitespace())
        .split(|c: char| !c.is_alphabetic() && c != '-')
        .next()
        .unwrap_or("")
        .to_lowercase()
}

/// The case a number takes after `word` (a preposition, an adjective or a
/// verb), if the word tells.
fn case_after(word: &str) -> Option<Case> {
    let case = match word {
        "до" | "от" | "из" | "около" | "более" | "менее" | "свыше" | "без" | "для" | "после"
        | "вместо" | "кроме" | "у" | "против" | "среди" | "с" | "со" | "порядка" | "начала"
        | "конца" | "середины" => Case::Gen,
        "к" | "ко" | "согласно" | "благодаря" => Case::Dat,
        "о" | "об" | "при" => Case::Prep,
        "в" | "во" | "на" | "за" | "про" | "через" | "сквозь" => Case::Acc,
        "между" | "над" | "под" | "перед" => Case::Ins,
        _ => {
            let ends = |suffixes: &[&str]| suffixes.iter().any(|s| word.ends_with(s));
            if word.chars().count() < 4 {
                return None;
            }
            if ends(&["ыми", "ими"]) {
                Case::Ins
            } else if ends(&["ых", "их"]) {
                Case::Gen
            } else if ends(&["ым", "им"]) {
                Case::Dat
            } else if ends(&[
                // Adjectives in the accusative and verb forms.
                "ую", "юю", "ил", "ал", "ял", "ел", "ла", "ли", "ло", "ив", "ав", "ят", "ет", "ит",
                "ют", "ут", "ать", "ить", "еть",
            ]) {
                Case::Acc
            } else {
                return None;
            }
        }
    };
    Some(case)
}

/// Gender of a bare number's noun guessed from the next word's ending in the
/// number's case: «одна акция», «одну акцию», «одной минуты», «две минуты»;
/// otherwise masculine. Only 1 and 2 change with gender.
fn gender_from_next(next: &str, n: u64, case: Case) -> Gender {
    let (last, teen) = (n % 10, (11..=14).contains(&(n % 100)));
    if teen || !matches!(last, 1 | 2) {
        return Gender::M;
    }
    let ends = |endings: &[&str]| endings.iter().any(|e| next.ends_with(e));
    match (last, case) {
        (1, Case::Nom) if ends(&["а", "я"]) => Gender::F,
        (1, Case::Nom) if ends(&["о", "е"]) => Gender::N,
        (1, Case::Acc) if ends(&["у", "ю"]) => Gender::F,
        (1, Case::Acc) if ends(&["о", "е"]) => Gender::N,
        (1, Case::Gen) if ends(&["ы", "и"]) => Gender::F,
        (1, Case::Dat) if ends(&["е", "и"]) => Gender::F,
        (1, Case::Ins) if ends(&["ой", "ей", "ью"]) => Gender::F,
        (2, Case::Nom | Case::Acc) if ends(&["и", "ы"]) => Gender::F,
        _ => Gender::M,
    }
}

fn digits(raw: &str) -> String {
    raw.chars().filter(char::is_ascii_digit).collect()
}

fn digit_by_digit(raw: &str) -> String {
    raw.chars()
        .filter_map(|c| c.to_digit(10))
        .map(|d| cardinal(d as u64, Case::Nom, Gender::M))
        .collect::<Vec<_>>()
        .join(" ")
}

fn roman(value: &str) -> Option<u64> {
    let digit = |c| match c {
        'I' => Some(1),
        'V' => Some(5),
        'X' => Some(10),
        'L' => Some(50),
        'C' => Some(100),
        _ => None,
    };
    let values: Vec<u64> = value.chars().map(digit).collect::<Option<_>>()?;
    let mut total = 0;
    for (i, v) in values.iter().enumerate() {
        if values.get(i + 1).is_some_and(|next| next > v) {
            total -= *v as i64;
        } else {
            total += *v as i64;
        }
    }
    (total > 0).then_some(total as u64)
}

/// Keep a sentence-ending dot that an abbreviation («млн.», «руб.») absorbed:
/// the dot is dropped only when the sentence clearly goes on.
fn keep_dot(after: &str) -> &'static str {
    let rest = after.trim_start();
    match rest.chars().next() {
        None => ".",
        Some(c) if c.is_lowercase() || c.is_ascii_digit() || c == ',' => "",
        Some(_) => ".",
    }
}

static DATE_NUMERIC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d{1,2})\.(\d{1,2})\.(\d{4})\b").unwrap());
static DATE_WORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"\b(\d{{1,2}})\s({})(?:\s(\d{{4}})(?:\s?(г\.|года\b))?)?",
        MONTHS_GEN.join("|")
    ))
    .unwrap()
});
static YEAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d{3,4})\s?(гг\.|г\.|год(?:а|у|ом|ах)?\b)").unwrap());
static CENTURY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([IVXLC]{1,6})\s(в\.|век(?:а|у|ом|е)?\b)").unwrap());
static ORDINAL_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(\d{1,4})-(ый|ий|ой|й|ая|я|ое|е|ые|ого|го|ому|му|ым|м|ых|х|ую|ю|ыми|ми)\b")
        .unwrap()
});
static TIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([01]?\d|2[0-3]):([0-5]\d)\b").unwrap());
static NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        (?:(?P<cur>[$€₽])\s?)?
        (?P<sign>[-−])?
        (?P<int>\d{1,3}(?:[\x20\u{00A0}]\d{3})+|\d+)
        (?:[,.](?P<frac>\d+))?
        (?:\s?(?P<scale>трлн|млрд|млн|тыс)\b(?P<sdot>\.)?)?
        (?:\s?(?:(?P<sym>%|₽|€|\$)|(?P<unit>км/ч|руб|долл|км|кг)\b(?P<udot>\.)?))?",
    )
    .unwrap()
});
static ABBREVIATION: LazyLock<Regex> = LazyLock::new(|| {
    let words: Vec<String> = ABBREVIATIONS
        .iter()
        .map(|(w, _)| regex::escape(w))
        .collect();
    Regex::new(&format!(r"\b(?:{})\b", words.join("|"))).unwrap()
});

fn dates_numeric(text: &str) -> String {
    DATE_NUMERIC
        .replace_all(text, |c: &Captures| {
            let whole = c.get(0).unwrap();
            let (d, m, y) = (
                c[1].parse::<u64>(),
                c[2].parse::<usize>(),
                c[3].parse::<u64>(),
            );
            match (d, m, y) {
                (Ok(d @ 1..=31), Ok(m @ 1..=12), Ok(y)) => {
                    let case = day_case(&previous_word(&text[..whole.start()]));
                    format!(
                        "{} {} {} года",
                        ordinal(d, case, Gender::N),
                        MONTHS_GEN[m - 1],
                        ordinal(y, Case::Gen, Gender::M)
                    )
                }
                _ => whole.as_str().to_string(),
            }
        })
        .into_owned()
}

/// Days are genitive in running text («20 ноября компания…»), except after
/// «на» (accusative) and «к» (dative).
fn day_case(prev: &str) -> Case {
    match prev {
        "на" | "в" => Case::Acc,
        "к" | "ко" => Case::Dat,
        _ => Case::Gen,
    }
}

fn dates_words(text: &str) -> String {
    DATE_WORDS
        .replace_all(text, |c: &Captures| {
            let whole = c.get(0).unwrap();
            let Ok(day @ 1..=31) = c[1].parse::<u64>() else {
                return whole.as_str().to_string();
            };
            let case = day_case(&previous_word(&text[..whole.start()]));
            let mut out = format!("{} {}", ordinal(day, case, Gender::N), &c[2]);
            if let Some(year) = c.get(3).and_then(|y| y.as_str().parse::<u64>().ok()) {
                out.push_str(&format!(" {} года", ordinal(year, Case::Gen, Gender::M)));
                if c.get(4).is_some_and(|g| g.as_str() == "г.") {
                    out.push_str(keep_dot(&text[whole.end()..]));
                }
            }
            out
        })
        .into_owned()
}

fn years(text: &str) -> String {
    YEAR_RE
        .replace_all(text, |c: &Captures| {
            let whole = c.get(0).unwrap();
            let Ok(year) = c[1].parse::<u64>() else {
                return whole.as_str().to_string();
            };
            let prev = previous_word(&text[..whole.start()]);
            let word = &c[2];
            let (case, noun) = match word {
                "год" => (Case::Nom, "год".to_string()),
                "года" => (Case::Gen, "года".to_string()),
                "годом" => (Case::Ins, "годом".to_string()),
                "годах" => (Case::Prep, "годах".to_string()),
                "году" => {
                    let case = if matches!(prev.as_str(), "к" | "ко") {
                        Case::Dat
                    } else {
                        Case::Prep
                    };
                    (case, "году".to_string())
                }
                "гг." => (
                    Case::Gen,
                    format!("годов{}", keep_dot(&text[whole.end()..])),
                ),
                _ => {
                    // «г.»: the preposition decides.
                    let case = match case_after(&prev) {
                        Some(Case::Acc) if matches!(prev.as_str(), "в" | "во") => Case::Prep,
                        Some(Case::Prep) => Case::Prep,
                        Some(Case::Dat) => Case::Dat,
                        _ => Case::Gen,
                    };
                    (
                        case,
                        format!(
                            "{}{}",
                            YEAR.sg[match case {
                                Case::Prep | Case::Dat => 2,
                                _ => 1,
                            }],
                            keep_dot(&text[whole.end()..])
                        ),
                    )
                }
            };
            format!("{} {noun}", ordinal(year, case, Gender::M))
        })
        .into_owned()
}

fn centuries(text: &str) -> String {
    CENTURY
        .replace_all(text, |c: &Captures| {
            let whole = c.get(0).unwrap();
            let Some(n) = roman(&c[1]) else {
                return whole.as_str().to_string();
            };
            let prev = previous_word(&text[..whole.start()]);
            let (case, noun) = match &c[2] {
                "века" => (Case::Gen, "века".to_string()),
                "веку" => (Case::Dat, "веку".to_string()),
                "веком" => (Case::Ins, "веком".to_string()),
                "веке" => (Case::Prep, "веке".to_string()),
                "век" => (Case::Nom, "век".to_string()),
                _ if matches!(prev.as_str(), "в" | "во" | "о" | "при") => (
                    Case::Prep,
                    format!("веке{}", keep_dot(&text[whole.end()..])),
                ),
                _ => (Case::Gen, format!("века{}", keep_dot(&text[whole.end()..]))),
            };
            format!("{} {noun}", ordinal(n, case, Gender::M))
        })
        .into_owned()
}

fn times(text: &str) -> String {
    TIME.replace_all(text, |c: &Captures| {
        let hours: u64 = c[1].parse().unwrap_or(0);
        let minutes: u64 = c[2].parse().unwrap_or(0);
        let m = match minutes {
            0 => "ноль-ноль".to_string(),
            1..=9 => format!("ноль {}", cardinal(minutes, Case::Nom, Gender::F)),
            _ => cardinal(minutes, Case::Nom, Gender::F),
        };
        format!("{} {m}", cardinal(hours, Case::Nom, Gender::M))
    })
    .into_owned()
}

/// «2-я» → «вторая», «90-е годы» → «девяностые годы», «в 90-х» → «в девяностых».
fn ordinal_suffixes(text: &str) -> String {
    ORDINAL_SUFFIX
        .replace_all(text, |c: &Captures| {
            let whole = c.get(0).unwrap();
            let Ok(n) = c[1].parse::<u64>() else {
                return whole.as_str().to_string();
            };
            let prev = previous_word(&text[..whole.start()]);
            let next = next_word(&text[whole.end()..]);
            let locative = matches!(prev.as_str(), "в" | "во" | "о" | "об" | "при" | "на");
            let plural_next =
                next.starts_with("год") && next != "год" && next != "года" && next != "году";
            match &c[2] {
                "ый" | "ий" | "ой" | "й" => ordinal(n, Case::Nom, Gender::M),
                "ая" | "я" => ordinal(n, Case::Nom, Gender::F),
                "ую" | "ю" => ordinal(n, Case::Acc, Gender::F),
                "ое" | "е" if plural_next || n >= 10 && n % 10 == 0 && next.is_empty() => {
                    ordinal_plural(n, Case::Nom)
                }
                "ое" | "е" => ordinal(n, Case::Nom, Gender::N),
                "ые" => ordinal_plural(n, Case::Nom),
                "ого" | "го" => ordinal(n, Case::Gen, Gender::M),
                "ому" | "му" => ordinal(n, Case::Dat, Gender::M),
                "ым" | "м" if locative => ordinal(n, Case::Prep, Gender::M),
                "ым" | "м" => ordinal(n, Case::Ins, Gender::M),
                "ых" | "х" if locative => ordinal_plural(n, Case::Prep),
                "ых" | "х" => ordinal_plural(n, Case::Gen),
                _ => ordinal_plural(n, Case::Ins),
            }
        })
        .into_owned()
}

fn numbers(text: &str) -> String {
    NUMBER
        .replace_all(text, |c: &Captures| {
            let whole = c.get(0).unwrap();
            let original = whole.as_str().to_string();
            let before = &text[..whole.start()];
            let after = &text[whole.end()..];
            // Parts of identifiers («5G», «A4», «COVID-19» keeps its number).
            if before.chars().last().is_some_and(|ch| ch.is_alphanumeric())
                || after.chars().next().is_some_and(|ch| ch.is_alphabetic())
                    && c.name("scale").is_none()
                    && c.name("sym").is_none()
                    && c.name("unit").is_none()
            {
                return original;
            }
            let raw = &c["int"];
            let int_digits = digits(raw);
            let negative = c.name("sign").is_some()
                && before
                    .chars()
                    .last()
                    .is_none_or(|ch| ch.is_whitespace() || ch == '(');
            if int_digits.len() > 15 || (int_digits.len() > 1 && int_digits.starts_with('0')) {
                return digit_by_digit(&int_digits);
            }
            let n: u64 = int_digits.parse().unwrap_or(0);
            let prev = previous_word(before);
            let next = next_word(after);
            let case = match case_after(&prev) {
                // «с 15 участниками» (творительный) vs «с 15 марта» (родительный).
                Some(Case::Gen)
                    if matches!(prev.as_str(), "с" | "со")
                        && ["ами", "ями", "ом", "ем", "ой", "ей"]
                            .iter()
                            .any(|e| next.ends_with(e)) =>
                {
                    Case::Ins
                }
                other => other.unwrap_or(Case::Nom),
            };
            let scale: Option<Noun> = c.name("scale").map(|s| match s.as_str() {
                "трлн" => TRILLION,
                "млрд" => BILLION,
                "млн" => MILLION,
                _ => THOUSAND,
            });
            let currency: Option<Noun> = c
                .name("cur")
                .or(c.name("sym"))
                .map(|s| s.as_str())
                .or(c.name("unit").map(|u| u.as_str()))
                .map(|s| match s {
                    "$" | "долл" => DOLLAR,
                    "км" | "км/ч" => KILOMETRE,
                    "кг" => KILOGRAM,
                    "€" => EURO,
                    "%" => PERCENT,
                    _ => RUBLE,
                });
            let mut words = Vec::new();
            if negative {
                words.push("минус".to_string());
            }
            match (c.name("frac").map(|f| f.as_str()), scale) {
                (Some("5"), Some(noun)) if n == 1 => words.push(one_and_a_half(case, &noun)),
                (Some("5"), None) if n == 1 && currency.is_some() => {
                    words.push(one_and_a_half(case, &currency.unwrap()))
                }
                (Some(frac), Some(noun)) => {
                    words.push(decimal(n, frac, case));
                    words.push(noun.sg[1].to_string());
                }
                (Some(frac), None) => {
                    words.push(decimal(n, frac, case));
                    if let Some(noun) = currency {
                        words.push(noun.sg[1].to_string());
                    }
                }
                (None, Some(noun)) => {
                    words.push(cardinal(n, case, noun.gender));
                    words.push(agree(n, case, &noun).to_string());
                }
                (None, None) => match currency {
                    Some(noun) => {
                        words.push(cardinal(n, case, noun.gender));
                        words.push(agree(n, case, &noun).to_string());
                    }
                    None => words.push(cardinal(n, case, gender_from_next(&next, n, case))),
                },
            }
            // After a scale word the currency is plural genitive: «миллиарда долларов».
            if scale.is_some() {
                if let Some(noun) = currency {
                    words.push(noun.pl[1].to_string());
                }
            }
            if c.name("unit").is_some_and(|u| u.as_str() == "км/ч") {
                words.push("в час".to_string());
            }
            let mut out = words.join(" ");
            if c.name("sdot").is_some() && c.name("sym").is_none() && c.name("unit").is_none()
                || c.name("udot").is_some()
            {
                out.push_str(keep_dot(after));
            }
            out
        })
        .into_owned()
}

fn abbreviations(text: &str) -> String {
    ABBREVIATION
        .replace_all(text, |c: &Captures| {
            ABBREVIATIONS
                .iter()
                .find(|(w, _)| *w == &c[0])
                .map_or(c[0].to_string(), |(_, r)| r.to_string())
        })
        .into_owned()
}

/// Normalize Russian text for speech.
pub fn normalize(text: &str) -> String {
    let text = dates_numeric(text);
    let text = dates_words(&text);
    let text = years(&text);
    let text = centuries(&text);
    let text = times(&text);
    let text = ordinal_suffixes(&text);
    let text = numbers(&text);
    abbreviations(&text)
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn news_example_from_the_user() {
        let text = "В ходе размещения разработчик чат‑бота Claude продал 980 млн акций вместо изначально запланированных 825 млн, выручив $81,3 млрд. Таким образом, это IPO стало крупнейшим в истории, превзойдя рекорд SpaceX в $75 млрд.";
        assert_eq!(
            normalize(text),
            "В ходе размещения разработчик чат‑бота Claude продал девятьсот восемьдесят миллионов акций вместо изначально запланированных восьмисот двадцати пяти миллионов, выручив восемьдесят одну целую три десятых миллиарда долларов. Таким образом, это ай-пи-о стало крупнейшим в истории, превзойдя рекорд спейс-икс в семьдесят пять миллиардов долларов."
        );
    }

    #[test]
    fn one_and_a_half_and_large_amounts() {
        assert_eq!(
            normalize("оценка составляет примерно $1,5 трлн."),
            "оценка составляет примерно полтора триллиона долларов."
        );
        assert_eq!(
            normalize("привлекла $65 млрд, однако"),
            "привлекла шестьдесят пять миллиардов долларов, однако"
        );
        assert_eq!(
            normalize("ниже отметки в $2 трлн, на которую"),
            "ниже отметки в два триллиона долларов, на которую"
        );
        assert_eq!(normalize("Бюджет 1 234 567 руб. в год."), "Бюджет один миллион двести тридцать четыре тысячи пятьсот шестьдесят семь рублей в год.");
        assert_eq!(normalize("стоит 5 тыс. руб."), "стоит пять тысяч рублей.");
        assert_eq!(
            normalize("около 3 млн человек"),
            "около трёх миллионов человек"
        );
        assert_eq!(normalize("Цена 1,5 руб."), "Цена полтора рубля.");
    }

    #[test]
    fn percentages_cases_and_genders() {
        assert_eq!(
            normalize("Рост на 25%."),
            "Рост на двадцать пять процентов."
        );
        assert_eq!(
            normalize("ставка 3,5% годовых"),
            "ставка три целых пять десятых процента годовых"
        );
        assert_eq!(normalize("до 21 процента"), "до двадцати одного процента");
        assert_eq!(normalize("купил 1 акцию"), "купил одну акцию");
        assert_eq!(normalize("Ждали 2 минуты."), "Ждали две минуты.");
        assert_eq!(normalize("с 15 участниками"), "с пятнадцатью участниками");
        assert_eq!(normalize("до 21 процента"), "до двадцати одного процента");
        assert_eq!(normalize("около 1 минуты"), "около одной минуты");
        assert_eq!(
            normalize("температура -5 градусов"),
            "температура минус пять градусов"
        );
    }

    #[test]
    fn dates_years_centuries_and_time() {
        assert_eq!(
            normalize("в 2026 году"),
            "в две тысячи двадцать шестом году"
        );
        assert_eq!(normalize("к 2030 году"), "к две тысячи тридцатому году");
        assert_eq!(normalize("до 2000 г. было"), "до двухтысячного года было");
        assert_eq!(
            normalize("В 1999 г. Москва"),
            "В тысяча девятьсот девяносто девятом году. Москва"
        );
        assert_eq!(
            normalize("20 ноября 2026 г. компания"),
            "двадцатого ноября две тысячи двадцать шестого года компания"
        );
        assert_eq!(normalize("на 3 марта"), "на третье марта");
        assert_eq!(
            normalize("Отчёт от 20.11.2026."),
            "Отчёт от двадцатого ноября две тысячи двадцать шестого года."
        );
        assert_eq!(normalize("в XXI веке"), "в двадцать первом веке");
        assert_eq!(
            normalize("Начало в 12:30, конец в 9:05."),
            "Начало в двенадцать тридцать, конец в девять ноль пять."
        );
    }

    #[test]
    fn identifiers_and_text_without_numbers_are_untouched() {
        for text in [
            "Сеть 5G и формат A4.",
            "Просто текст без чисел, т. е. ничего.",
            "Код 007 агента.",
        ] {
            let out = normalize(text);
            if text.contains("007") {
                assert_eq!(out, "Код ноль ноль семь агента.");
            } else {
                assert_eq!(out, text);
            }
        }
        assert_eq!(normalize("ВВП США вырос"), "вэ-вэ-пэ сэ-шэ-а вырос");
    }
}

#[cfg(test)]
mod corpus {
    use super::normalize;

    /// News-style phrases and how they must be read.
    const CASES: &[(&str, &str)] = &[
        (
            "Курс доллара 92,5 руб.",
            "Курс доллара девяносто две целых пять десятых рубля.",
        ),
        (
            "Инфляция составила 7,4% за год.",
            "Инфляция составила семь целых четыре десятых процента за год.",
        ),
        (
            "В 2025 г. выручка выросла на 12%.",
            "В две тысячи двадцать пятом году выручка выросла на двенадцать процентов.",
        ),
        (
            "Компания наняла 1 500 сотрудников.",
            "Компания наняла тысячу пятьсот сотрудников.",
        ),
        (
            "Акции подорожали на 0,8%.",
            "Акции подорожали на ноль целых восемь десятых процента.",
        ),
        ("К 15 декабря.", "К пятнадцатому декабря."),
        (
            "С 1 января 2027 года.",
            "С первого января две тысячи двадцать седьмого года.",
        ),
        (
            "Прибыль — 3,2 млрд руб.",
            "Прибыль — три целых две десятых миллиарда рублей.",
        ),
        ("Более 100 тыс. человек.", "Более ста тысяч человек."),
        ("Около 40 стран.", "Около сорока стран."),
        (
            "Скорость 120 км/ч.",
            "Скорость сто двадцать километров в час.",
        ),
        ("Дистанция 42 км.", "Дистанция сорок два километра."),
        ("Вес 1 кг.", "Вес один килограмм."),
        ("Ему 21 год.", "Ему двадцать один год."),
        (
            "Ей исполнилось 22 года.",
            "Ей исполнилось двадцать два года.",
        ),
        ("Рейс в 06:15.", "Рейс в шесть пятнадцать."),
        ("Это 2-я попытка.", "Это вторая попытка."),
        ("В 90-е годы.", "В девяностые годы."),
        ("В 90-х годах.", "В девяностых годах."),
        ("Занял 3-е место.", "Занял третье место."),
        ("На 5-м этаже.", "На пятом этаже."),
        (
            "Сделка на $2,4 млн.",
            "Сделка на две целых четыре десятых миллиона долларов.",
        ),
        ("Потери 1 млн долларов.", "Потери один миллион долларов."),
        ("За 3 дня.", "За три дня."),
        ("Через 5 лет.", "Через пять лет."),
        ("На 1 мая.", "На первое мая."),
        (
            "Около 1 000 000 зрителей.",
            "Около одного миллиона зрителей.",
        ),
        ("Её доля 51%.", "Её доля пятьдесят один процент."),
        ("Курс евро 100 €.", "Курс евро сто евро."),
        (
            "На 2,5 трлн руб.",
            "На две целых пять десятых триллиона рублей.",
        ),
        (
            "Население 146 млн человек.",
            "Население сто сорок шесть миллионов человек.",
        ),
        (
            "По данным на 01.10.2026.",
            "По данным на первое октября две тысячи двадцать шестого года.",
        ),
        ("В XIX в. город рос.", "В девятнадцатом веке город рос."),
        (
            "Обошлось в 1,5 млрд евро.",
            "Обошлось в полтора миллиарда евро.",
        ),
        (
            "Из 27 участников 3 вышли.",
            "Из двадцати семи участников три вышли.",
        ),
        ("Бюджет 2026 г.", "Бюджет две тысячи двадцать шестого года."),
    ];

    #[test]
    fn news_phrases_read_naturally() {
        for (input, expected) in CASES {
            assert_eq!(&normalize(input), expected, "{input}");
        }
    }
}
