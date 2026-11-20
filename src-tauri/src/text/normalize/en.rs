//! English text normalization for speech: numbers, money, percentages,
//! ordinals and years become words. Offline; unknown patterns are left alone.

use std::sync::LazyLock;

use regex::{Captures, Regex};

const ONES: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

fn below_thousand(n: u64) -> String {
    let mut parts = Vec::new();
    if n >= 100 {
        parts.push(format!("{} hundred", ONES[(n / 100) as usize]));
    }
    let rest = n % 100;
    if rest > 0 || n == 0 {
        parts.push(if rest < 20 {
            ONES[rest as usize].to_string()
        } else if rest.is_multiple_of(10) {
            TENS[(rest / 10) as usize].to_string()
        } else {
            format!(
                "{}-{}",
                TENS[(rest / 10) as usize],
                ONES[(rest % 10) as usize]
            )
        });
    }
    parts.join(" ")
}

/// A cardinal number (n < 10^15) in English words.
pub fn cardinal(n: u64) -> String {
    if n == 0 {
        return "zero".into();
    }
    let scales = [
        (1_000_000_000_000, "trillion"),
        (1_000_000_000, "billion"),
        (1_000_000, "million"),
        (1_000, "thousand"),
    ];
    let mut parts = Vec::new();
    let mut rest = n;
    for (size, name) in scales {
        if rest >= size {
            parts.push(format!("{} {name}", below_thousand(rest / size)));
            rest %= size;
        }
    }
    if rest > 0 {
        parts.push(below_thousand(rest));
    }
    parts.join(" ")
}

/// An ordinal: 1 → «first», 22 → «twenty-second».
pub fn ordinal(n: u64) -> String {
    let words = cardinal(n);
    let (head, last) = match words.rfind([' ', '-']) {
        Some(i) => (&words[..=i], &words[i + 1..]),
        None => ("", words.as_str()),
    };
    let last = match last {
        "one" => "first".to_string(),
        "two" => "second".to_string(),
        "three" => "third".to_string(),
        "five" => "fifth".to_string(),
        "eight" => "eighth".to_string(),
        "nine" => "ninth".to_string(),
        "twelve" => "twelfth".to_string(),
        w if w.ends_with('y') => format!("{}ieth", &w[..w.len() - 1]),
        w => format!("{w}th"),
    };
    format!("{head}{last}")
}

/// A year: 1999 → «nineteen ninety-nine», 2006 → «two thousand six»,
/// 2026 → «twenty twenty-six».
pub fn year(n: u64) -> String {
    if (2000..2010).contains(&n) || n.is_multiple_of(1000) || !(1100..2100).contains(&n) {
        return cardinal(n);
    }
    let (high, low) = (n / 100, n % 100);
    match low {
        0 => format!("{} hundred", cardinal(high)),
        1..=9 => format!("{} oh {}", cardinal(high), cardinal(low)),
        _ => format!("{} {}", cardinal(high), cardinal(low)),
    }
}

static AMOUNT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?xi)
        (?:(?P<cur>[$€£])\s?)?
        (?P<int>\d{1,3}(?:,\d{3})+|\d+)
        (?:\.(?P<frac>\d+))?
        (?:\s?(?P<scale>trillion|billion|million|thousand|tn|bn|m|k)\b)?
        (?P<pct>%)?",
    )
    .unwrap()
});
static ORDINAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(\d+)(st|nd|rd|th)\b").unwrap());
static YEAR_CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(in|since|by|from|until|of|year|during|after|before)\s(\d{4})\b").unwrap()
});

fn currency_name(symbol: &str, plural: bool) -> &'static str {
    match (symbol, plural) {
        ("$", true) => "dollars",
        ("$", false) => "dollar",
        ("€", _) => "euros",
        ("£", true) => "pounds",
        ("£", false) => "pound",
        _ => "",
    }
}

/// Normalize English text for speech.
pub fn normalize(text: &str) -> String {
    let text = ORDINAL
        .replace_all(text, |c: &Captures| {
            c[1].parse::<u64>().map_or(c[0].to_string(), ordinal)
        })
        .into_owned();
    let text = YEAR_CONTEXT
        .replace_all(&text, |c: &Captures| {
            let n: u64 = c[2].parse().unwrap_or(0);
            format!("{} {}", &c[1], year(n))
        })
        .into_owned();
    AMOUNT
        .replace_all(&text, |c: &Captures| {
            let whole = c.get(0).unwrap();
            let before = &text[..whole.start()];
            let after = &text[whole.end()..];
            if before.chars().last().is_some_and(|ch| ch.is_alphanumeric())
                || after.chars().next().is_some_and(|ch| ch.is_alphabetic())
            {
                return whole.as_str().to_string();
            }
            let int: String = c["int"].chars().filter(char::is_ascii_digit).collect();
            if int.len() > 15 {
                return whole.as_str().to_string();
            }
            // «5 m» is five metres, not five million: m/k only with a currency.
            if c.name("cur").is_none()
                && c.name("scale")
                    .is_some_and(|s| matches!(s.as_str().to_lowercase().as_str(), "m" | "k"))
            {
                return whole.as_str().to_string();
            }
            let n: u64 = int.parse().unwrap_or(0);
            let mut words = cardinal(n);
            if let Some(frac) = c.name("frac") {
                let digits: Vec<&str> = frac
                    .as_str()
                    .chars()
                    .filter_map(|d| d.to_digit(10))
                    .map(|d| ONES[d as usize])
                    .collect();
                words = format!("{words} point {}", digits.join(" "));
            }
            if let Some(scale) = c.name("scale") {
                let name = match scale.as_str().to_lowercase().as_str() {
                    "tn" | "trillion" => "trillion",
                    "bn" | "billion" => "billion",
                    "m" | "million" => "million",
                    _ => "thousand",
                };
                words = format!("{words} {name}");
            }
            if c.name("pct").is_some() {
                words.push_str(" percent");
            }
            if let Some(cur) = c.name("cur") {
                let plural = !(n == 1 && c.name("frac").is_none() && c.name("scale").is_none());
                words = format!("{words} {}", currency_name(cur.as_str(), plural));
            }
            words
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_money_and_percentages() {
        assert_eq!(
            normalize("The company sold 980 million shares, raising $81.3 billion."),
            "The company sold nine hundred eighty million shares, raising eighty-one point three billion dollars."
        );
        assert_eq!(
            normalize("A record of $75bn."),
            "A record of seventy-five billion dollars."
        );
        assert_eq!(
            normalize("Growth was 3.5% in 1,250 stores."),
            "Growth was three point five percent in one thousand two hundred fifty stores."
        );
        assert_eq!(normalize("It costs $1."), "It costs one dollar.");
        assert_eq!(normalize("Price: €20"), "Price: twenty euros");
    }

    #[test]
    fn years_and_ordinals() {
        assert_eq!(
            normalize("in 2026 and since 1999"),
            "in twenty twenty-six and since nineteen ninety-nine"
        );
        assert_eq!(normalize("by 2005"), "by two thousand five");
        assert_eq!(
            normalize("the 21st century, the 3rd time"),
            "the twenty-first century, the third time"
        );
        assert_eq!(normalize("on the 12th"), "on the twelfth");
        assert_eq!(normalize("in 1905"), "in nineteen oh five");
    }

    #[test]
    fn identifiers_stay() {
        assert_eq!(normalize("5G and A4 paper"), "5G and A4 paper");
        assert_eq!(normalize("Plain text."), "Plain text.");
        assert_eq!(normalize("a 5 m wall"), "a 5 m wall");
        assert_eq!(normalize("raised $5m"), "raised five million dollars");
    }
}
