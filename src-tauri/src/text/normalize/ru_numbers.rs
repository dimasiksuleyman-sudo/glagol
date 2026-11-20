//! Russian numerals: cardinals and ordinals in every case and gender,
//! decimals and noun agreement («один миллион», «два миллиона», «пять
//! миллионов», «двух миллионов»).

/// Grammatical case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// Именительный.
    Nom,
    /// Родительный.
    Gen,
    /// Дательный.
    Dat,
    /// Винительный (inanimate).
    Acc,
    /// Творительный.
    Ins,
    /// Предложный.
    Prep,
}

impl Case {
    fn index(self) -> usize {
        match self {
            Case::Nom => 0,
            Case::Gen => 1,
            Case::Dat => 2,
            Case::Acc => 3,
            Case::Ins => 4,
            Case::Prep => 5,
        }
    }

    fn is_direct(self) -> bool {
        matches!(self, Case::Nom | Case::Acc)
    }
}

/// Grammatical gender of the counted noun.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gender {
    /// Мужской.
    M,
    /// Женский.
    F,
    /// Средний.
    N,
}

/// A noun's singular and plural forms in the six cases (Nom, Gen, Dat, Acc,
/// Ins, Prep).
#[derive(Clone, Copy, Debug)]
pub struct Noun {
    /// Singular forms.
    pub sg: [&'static str; 6],
    /// Plural forms.
    pub pl: [&'static str; 6],
    /// Gender, for «один/одна».
    pub gender: Gender,
}

pub const THOUSAND: Noun = Noun {
    sg: ["тысяча", "тысячи", "тысяче", "тысячу", "тысячей", "тысяче"],
    pl: [
        "тысячи",
        "тысяч",
        "тысячам",
        "тысячи",
        "тысячами",
        "тысячах",
    ],
    gender: Gender::F,
};
pub const MILLION: Noun = Noun {
    sg: [
        "миллион",
        "миллиона",
        "миллиону",
        "миллион",
        "миллионом",
        "миллионе",
    ],
    pl: [
        "миллионы",
        "миллионов",
        "миллионам",
        "миллионы",
        "миллионами",
        "миллионах",
    ],
    gender: Gender::M,
};
pub const BILLION: Noun = Noun {
    sg: [
        "миллиард",
        "миллиарда",
        "миллиарду",
        "миллиард",
        "миллиардом",
        "миллиарде",
    ],
    pl: [
        "миллиарды",
        "миллиардов",
        "миллиардам",
        "миллиарды",
        "миллиардами",
        "миллиардах",
    ],
    gender: Gender::M,
};
pub const TRILLION: Noun = Noun {
    sg: [
        "триллион",
        "триллиона",
        "триллиону",
        "триллион",
        "триллионом",
        "триллионе",
    ],
    pl: [
        "триллионы",
        "триллионов",
        "триллионам",
        "триллионы",
        "триллионами",
        "триллионах",
    ],
    gender: Gender::M,
};
pub const DOLLAR: Noun = Noun {
    sg: [
        "доллар",
        "доллара",
        "доллару",
        "доллар",
        "долларом",
        "долларе",
    ],
    pl: [
        "доллары",
        "долларов",
        "долларам",
        "доллары",
        "долларами",
        "долларах",
    ],
    gender: Gender::M,
};
pub const RUBLE: Noun = Noun {
    sg: ["рубль", "рубля", "рублю", "рубль", "рублём", "рубле"],
    pl: ["рубли", "рублей", "рублям", "рубли", "рублями", "рублях"],
    gender: Gender::M,
};
pub const EURO: Noun = Noun {
    sg: ["евро"; 6],
    pl: ["евро"; 6],
    gender: Gender::N,
};
pub const PERCENT: Noun = Noun {
    sg: [
        "процент",
        "процента",
        "проценту",
        "процент",
        "процентом",
        "проценте",
    ],
    pl: [
        "проценты",
        "процентов",
        "процентам",
        "проценты",
        "процентами",
        "процентах",
    ],
    gender: Gender::M,
};
pub const KILOMETRE: Noun = Noun {
    sg: [
        "километр",
        "километра",
        "километру",
        "километр",
        "километром",
        "километре",
    ],
    pl: [
        "километры",
        "километров",
        "километрам",
        "километры",
        "километрами",
        "километрах",
    ],
    gender: Gender::M,
};
pub const KILOGRAM: Noun = Noun {
    sg: [
        "килограмм",
        "килограмма",
        "килограмму",
        "килограмм",
        "килограммом",
        "килограмме",
    ],
    pl: [
        "килограммы",
        "килограммов",
        "килограммам",
        "килограммы",
        "килограммами",
        "килограммах",
    ],
    gender: Gender::M,
};
pub const YEAR: Noun = Noun {
    sg: ["год", "года", "году", "год", "годом", "году"],
    pl: ["годы", "лет", "годам", "годы", "годами", "годах"],
    gender: Gender::M,
};
const WHOLE: Noun = Noun {
    sg: ["целая", "целой", "целой", "целую", "целой", "целой"],
    pl: ["целых", "целых", "целым", "целых", "целыми", "целых"],
    gender: Gender::F,
};
const TENTH: Noun = Noun {
    sg: [
        "десятая",
        "десятой",
        "десятой",
        "десятую",
        "десятой",
        "десятой",
    ],
    pl: [
        "десятых",
        "десятых",
        "десятым",
        "десятых",
        "десятыми",
        "десятых",
    ],
    gender: Gender::F,
};
const HUNDREDTH: Noun = Noun {
    sg: ["сотая", "сотой", "сотой", "сотую", "сотой", "сотой"],
    pl: ["сотых", "сотых", "сотым", "сотых", "сотыми", "сотых"],
    gender: Gender::F,
};
const THOUSANDTH: Noun = Noun {
    sg: [
        "тысячная",
        "тысячной",
        "тысячной",
        "тысячную",
        "тысячной",
        "тысячной",
    ],
    pl: [
        "тысячных",
        "тысячных",
        "тысячным",
        "тысячных",
        "тысячными",
        "тысячных",
    ],
    gender: Gender::F,
};

/// The form of `noun` that follows the number `n` in `case`.
pub fn agree(n: u64, case: Case, noun: &Noun) -> &'static str {
    let (last, last_two) = (n % 10, n % 100);
    let teen = (11..=14).contains(&last_two);
    if case.is_direct() {
        if last == 1 && !teen {
            noun.sg[case.index()]
        } else if (2..=4).contains(&last) && !teen {
            noun.sg[1]
        } else {
            noun.pl[1]
        }
    } else if last == 1 && !teen {
        noun.sg[case.index()]
    } else {
        noun.pl[case.index()]
    }
}

fn units(n: u64, case: Case, gender: Gender) -> String {
    let i = case.index();
    let table: [&str; 6] = match (n, gender) {
        (1, Gender::M) => ["один", "одного", "одному", "один", "одним", "одном"],
        (1, Gender::F) => ["одна", "одной", "одной", "одну", "одной", "одной"],
        (1, Gender::N) => ["одно", "одного", "одному", "одно", "одним", "одном"],
        (2, Gender::F) => ["две", "двух", "двум", "две", "двумя", "двух"],
        (2, _) => ["два", "двух", "двум", "два", "двумя", "двух"],
        (3, _) => ["три", "трёх", "трём", "три", "тремя", "трёх"],
        (4, _) => [
            "четыре",
            "четырёх",
            "четырём",
            "четыре",
            "четырьмя",
            "четырёх",
        ],
        (8, _) => ["восемь", "восьми", "восьми", "восемь", "восемью", "восьми"],
        (40, _) => ["сорок", "сорока", "сорока", "сорок", "сорока", "сорока"],
        (90, _) => [
            "девяносто",
            "девяноста",
            "девяноста",
            "девяносто",
            "девяноста",
            "девяноста",
        ],
        (100, _) => ["сто", "ста", "ста", "сто", "ста", "ста"],
        (50, _) => [
            "пятьдесят",
            "пятидесяти",
            "пятидесяти",
            "пятьдесят",
            "пятьюдесятью",
            "пятидесяти",
        ],
        (60, _) => [
            "шестьдесят",
            "шестидесяти",
            "шестидесяти",
            "шестьдесят",
            "шестьюдесятью",
            "шестидесяти",
        ],
        (70, _) => [
            "семьдесят",
            "семидесяти",
            "семидесяти",
            "семьдесят",
            "семьюдесятью",
            "семидесяти",
        ],
        (80, _) => [
            "восемьдесят",
            "восьмидесяти",
            "восьмидесяти",
            "восемьдесят",
            "восемьюдесятью",
            "восьмидесяти",
        ],
        (200, _) => [
            "двести",
            "двухсот",
            "двумстам",
            "двести",
            "двумястами",
            "двухстах",
        ],
        (300, _) => [
            "триста",
            "трёхсот",
            "трёмстам",
            "триста",
            "тремястами",
            "трёхстах",
        ],
        (400, _) => [
            "четыреста",
            "четырёхсот",
            "четырёмстам",
            "четыреста",
            "четырьмястами",
            "четырёхстах",
        ],
        (500, _) => [
            "пятьсот",
            "пятисот",
            "пятистам",
            "пятьсот",
            "пятьюстами",
            "пятистах",
        ],
        (600, _) => [
            "шестьсот",
            "шестисот",
            "шестистам",
            "шестьсот",
            "шестьюстами",
            "шестистах",
        ],
        (700, _) => [
            "семьсот",
            "семисот",
            "семистам",
            "семьсот",
            "семьюстами",
            "семистах",
        ],
        (800, _) => [
            "восемьсот",
            "восьмисот",
            "восьмистам",
            "восемьсот",
            "восемьюстами",
            "восьмистах",
        ],
        (900, _) => [
            "девятьсот",
            "девятисот",
            "девятистам",
            "девятьсот",
            "девятьюстами",
            "девятистах",
        ],
        _ => {
            // 5–7, 9–20, 30: nominative ends in «ь».
            let nom = match n {
                5 => "пять",
                6 => "шесть",
                7 => "семь",
                9 => "девять",
                10 => "десять",
                11 => "одиннадцать",
                12 => "двенадцать",
                13 => "тринадцать",
                14 => "четырнадцать",
                15 => "пятнадцать",
                16 => "шестнадцать",
                17 => "семнадцать",
                18 => "восемнадцать",
                19 => "девятнадцать",
                20 => "двадцать",
                30 => "тридцать",
                _ => unreachable!("units() takes 1–20, tens and hundreds"),
            };
            let stem = &nom[..nom.len() - "ь".len()];
            return match case {
                Case::Nom | Case::Acc => nom.to_string(),
                Case::Ins => format!("{stem}ью"),
                _ => format!("{stem}и"),
            };
        }
    };
    table[i].to_string()
}

/// 1–999 as words.
fn triplet(n: u64, case: Case, gender: Gender) -> Vec<String> {
    let mut words = Vec::new();
    let (h, rest) = (n / 100 * 100, n % 100);
    if h > 0 {
        words.push(units(h, case, gender));
    }
    if rest > 0 {
        if rest <= 20 {
            words.push(units(rest, case, gender));
        } else {
            words.push(units(rest / 10 * 10, case, gender));
            if rest % 10 > 0 {
                words.push(units(rest % 10, case, gender));
            }
        }
    }
    words
}

/// A cardinal number (0 ≤ n < 10^15) in `case`, agreeing with `gender`.
pub fn cardinal(n: u64, case: Case, gender: Gender) -> String {
    if n == 0 {
        return match case {
            Case::Gen => "нуля",
            Case::Dat => "нулю",
            Case::Ins => "нулём",
            Case::Prep => "нуле",
            _ => "ноль",
        }
        .to_string();
    }
    let scales = [
        (1_000_000_000_000, TRILLION),
        (1_000_000_000, BILLION),
        (1_000_000, MILLION),
        (1_000, THOUSAND),
    ];
    let mut words = Vec::new();
    let mut rest = n;
    for (size, noun) in scales {
        let count = rest / size;
        if count > 0 {
            // «тысяча пятьсот», not «одна тысяча пятьсот» — unless the
            // number is exactly one thousand.
            if !(size == 1_000 && count == 1 && words.is_empty() && !rest.is_multiple_of(size)) {
                words.extend(triplet(count, case, noun.gender));
            }
            words.push(agree(count, case, &noun).to_string());
            rest %= size;
        }
    }
    if rest > 0 {
        words.extend(triplet(rest, case, gender));
    }
    words.join(" ")
}

enum Stem {
    Hard(&'static str),
    Stressed(&'static str),
    Third,
}

fn ordinal_stem(n: u64) -> Stem {
    use Stem::*;
    match n {
        1 => Hard("перв"),
        2 => Stressed("втор"),
        3 => Third,
        4 => Hard("четвёрт"),
        5 => Hard("пят"),
        6 => Stressed("шест"),
        7 => Stressed("седьм"),
        8 => Stressed("восьм"),
        9 => Hard("девят"),
        10 => Hard("десят"),
        11 => Hard("одиннадцат"),
        12 => Hard("двенадцат"),
        13 => Hard("тринадцат"),
        14 => Hard("четырнадцат"),
        15 => Hard("пятнадцат"),
        16 => Hard("шестнадцат"),
        17 => Hard("семнадцат"),
        18 => Hard("восемнадцат"),
        19 => Hard("девятнадцат"),
        20 => Hard("двадцат"),
        30 => Hard("тридцат"),
        40 => Stressed("сороков"),
        50 => Hard("пятидесят"),
        60 => Hard("шестидесят"),
        70 => Hard("семидесят"),
        80 => Hard("восьмидесят"),
        90 => Hard("девяност"),
        100 => Hard("сот"),
        200 => Hard("двухсот"),
        300 => Hard("трёхсот"),
        400 => Hard("четырёхсот"),
        500 => Hard("пятисот"),
        600 => Hard("шестисот"),
        700 => Hard("семисот"),
        800 => Hard("восьмисот"),
        900 => Hard("девятисот"),
        _ => unreachable!("ordinal stems are 1–20, tens and hundreds"),
    }
}

fn ordinal_word(stem: Stem, case: Case, gender: Gender) -> String {
    let (base, endings) = ordinal_parts(stem, gender);
    format!("{base}{}", endings[case.index()])
}

fn ordinal_parts(stem: Stem, gender: Gender) -> (&'static str, [&'static str; 6]) {
    match (stem, gender) {
        (Stem::Third, Gender::F) => ("трет", ["ья", "ьей", "ьей", "ью", "ьей", "ьей"]),
        (Stem::Third, Gender::N) => ("трет", ["ье", "ьего", "ьему", "ье", "ьим", "ьем"]),
        (Stem::Third, Gender::M) => ("трет", ["ий", "ьего", "ьему", "ий", "ьим", "ьем"]),
        (Stem::Hard(s) | Stem::Stressed(s), Gender::F) => (s, ["ая", "ой", "ой", "ую", "ой", "ой"]),
        (Stem::Hard(s) | Stem::Stressed(s), Gender::N) => {
            (s, ["ое", "ого", "ому", "ое", "ым", "ом"])
        }
        (Stem::Hard(s), Gender::M) => (s, ["ый", "ого", "ому", "ый", "ым", "ом"]),
        (Stem::Stressed(s), Gender::M) => (s, ["ой", "ого", "ому", "ой", "ым", "ом"]),
    }
}

/// A plural ordinal: «девяностые», «в девяностых».
pub fn ordinal_plural(n: u64, case: Case) -> String {
    let singular = ordinal(n, Case::Nom, Gender::M);
    let (head, last) = singular
        .rsplit_once(' ')
        .map_or(("", singular.as_str()), |(h, l)| (h, l));
    let stem = last
        .strip_suffix("ий")
        .map(|s| (s, ["ьи", "ьих", "ьим", "ьи", "ьими", "ьих"]))
        .or_else(|| {
            last.strip_suffix("ый")
                .or_else(|| last.strip_suffix("ой"))
                .map(|s| (s, ["ые", "ых", "ым", "ые", "ыми", "ых"]))
        });
    match stem {
        Some((base, endings)) => {
            let word = format!("{base}{}", endings[case.index()]);
            if head.is_empty() {
                word
            } else {
                format!("{head} {word}")
            }
        }
        None => singular,
    }
}

/// An ordinal number (1 ≤ n < 10^6): «две тысячи двадцать шестом».
pub fn ordinal(n: u64, case: Case, gender: Gender) -> String {
    if n == 0 || n >= 1_000_000 {
        return cardinal(n, Case::Nom, Gender::M);
    }
    if n.is_multiple_of(1000) {
        // 1000 → «тысячный», 2000 → «двухтысячный».
        let k = n / 1000;
        let prefix = if k == 1 {
            String::new()
        } else if k < 10 {
            units(k, Case::Gen, Gender::F)
        } else {
            return format!(
                "{} {}",
                cardinal(k, Case::Nom, Gender::F),
                ordinal_word(Stem::Hard("тысячн"), case, gender)
            );
        };
        let (_, endings) = ordinal_parts(Stem::Hard(""), gender);
        return format!("{prefix}тысячн{}", endings[case.index()]);
    }
    let thousands = n / 1000 * 1000;
    let rest = n % 1000;
    let mut words = Vec::new();
    if thousands > 0 {
        // Years read «тысяча девятьсот…», not «одна тысяча…».
        let count = cardinal(thousands, Case::Nom, Gender::M);
        words.push(
            count
                .strip_prefix("одна ")
                .map(str::to_string)
                .unwrap_or(count),
        );
    }
    let (h, tail) = (rest / 100 * 100, rest % 100);
    let last = if tail == 0 {
        h
    } else if tail <= 20 || tail % 10 == 0 {
        if h > 0 {
            words.push(units(h, Case::Nom, Gender::M));
        }
        tail
    } else {
        if h > 0 {
            words.push(units(h, Case::Nom, Gender::M));
        }
        words.push(units(tail / 10 * 10, Case::Nom, Gender::M));
        tail % 10
    };
    words.push(ordinal_word(ordinal_stem(last), case, gender));
    words.join(" ")
}

/// A decimal number «81,3» → «восемьдесят одна целая три десятых». Up to three
/// fraction digits; longer fractions are read digit by digit.
pub fn decimal(whole: u64, fraction: &str, case: Case) -> String {
    let denominator = match fraction.len() {
        1 => TENTH,
        2 => HUNDREDTH,
        3 => THOUSANDTH,
        _ => {
            let digits: Vec<String> = fraction
                .chars()
                .filter_map(|c| c.to_digit(10))
                .map(|d| cardinal(d as u64, Case::Nom, Gender::M))
                .collect();
            return format!(
                "{} запятая {}",
                cardinal(whole, case, Gender::M),
                digits.join(" ")
            );
        }
    };
    let numerator: u64 = fraction.parse().unwrap_or(0);
    format!(
        "{} {} {} {}",
        cardinal(whole, case, Gender::F),
        agree_fraction(whole, case, &WHOLE),
        cardinal(numerator, case, Gender::F),
        agree_fraction(numerator, case, &denominator)
    )
}

/// «Целая»/«десятая» are adjectives: after 2–4 they take the plural
/// («три целых», «две десятых»), unlike nouns («три миллиона»).
fn agree_fraction(n: u64, case: Case, noun: &Noun) -> &'static str {
    let (last, teen) = (n % 10, (11..=14).contains(&(n % 100)));
    if last == 1 && !teen {
        noun.sg[case.index()]
    } else if case.is_direct() {
        noun.pl[1]
    } else {
        noun.pl[case.index()]
    }
}

/// «1,5» before a noun: «полтора миллиона», «полторы тысячи», «полутора миллионов».
pub fn one_and_a_half(case: Case, noun: &Noun) -> String {
    let word = match (case.is_direct(), noun.gender) {
        (true, Gender::F) => "полторы",
        (true, _) => "полтора",
        (false, _) => "полутора",
    };
    let noun_form = if case.is_direct() {
        noun.sg[1]
    } else {
        noun.pl[case.index()]
    };
    format!("{word} {noun_form}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use Case::*;
    use Gender::*;

    #[test]
    fn cardinals_decline() {
        assert_eq!(cardinal(980, Nom, M), "девятьсот восемьдесят");
        assert_eq!(cardinal(825, Gen, M), "восьмисот двадцати пяти");
        assert_eq!(cardinal(21, Acc, F), "двадцать одну");
        assert_eq!(cardinal(2, Nom, F), "две");
        assert_eq!(
            cardinal(1_234_567, Nom, M),
            "один миллион двести тридцать четыре тысячи пятьсот шестьдесят семь"
        );
        assert_eq!(cardinal(5_000, Gen, M), "пяти тысяч");
        assert_eq!(cardinal(2_000_000, Ins, M), "двумя миллионами");
        assert_eq!(cardinal(48, Ins, M), "сорока восемью");
        assert_eq!(cardinal(0, Nom, M), "ноль");
        assert_eq!(cardinal(11, Prep, M), "одиннадцати");
        assert_eq!(cardinal(1_000, Nom, M), "одна тысяча");
        assert_eq!(cardinal(1_500, Acc, M), "тысячу пятьсот");
        assert_eq!(ordinal_plural(90, Nom), "девяностые");
        assert_eq!(ordinal_plural(90, Prep), "девяностых");
        assert_eq!(ordinal_plural(3, Nom), "третьи");
    }

    #[test]
    fn nouns_agree_with_numbers() {
        assert_eq!(agree(1, Nom, &MILLION), "миллион");
        assert_eq!(agree(3, Nom, &MILLION), "миллиона");
        assert_eq!(agree(980, Nom, &MILLION), "миллионов");
        assert_eq!(agree(12, Nom, &DOLLAR), "долларов");
        assert_eq!(agree(21, Nom, &RUBLE), "рубль");
        assert_eq!(agree(825, Gen, &MILLION), "миллионов");
        assert_eq!(agree(1, Acc, &THOUSAND), "тысячу");
        assert_eq!(agree(5, Dat, &PERCENT), "процентам");
    }

    #[test]
    fn ordinals_for_years_and_dates() {
        assert_eq!(ordinal(2026, Prep, M), "две тысячи двадцать шестом");
        assert_eq!(ordinal(2000, Gen, M), "двухтысячного");
        assert_eq!(ordinal(1999, Nom, M), "тысяча девятьсот девяносто девятый");
        assert_eq!(ordinal(20, Gen, N), "двадцатого");
        assert_eq!(ordinal(3, Nom, N), "третье");
        assert_eq!(ordinal(21, Nom, M), "двадцать первый");
        assert_eq!(ordinal(40, Prep, M), "сороковом");
        assert_eq!(ordinal(2100, Nom, M), "две тысячи сотый");
    }

    #[test]
    fn decimals_and_one_and_a_half() {
        assert_eq!(decimal(81, "3", Nom), "восемьдесят одна целая три десятых");
        assert_eq!(decimal(81, "3", Acc), "восемьдесят одну целую три десятых");
        assert_eq!(decimal(2, "75", Nom), "две целых семьдесят пять сотых");
        assert_eq!(decimal(0, "5", Gen), "нуля целых пяти десятых");
        assert_eq!(one_and_a_half(Nom, &TRILLION), "полтора триллиона");
        assert_eq!(one_and_a_half(Nom, &THOUSAND), "полторы тысячи");
        assert_eq!(one_and_a_half(Gen, &MILLION), "полутора миллионов");
    }
}
