//! Localize legacy string errors at IPC/event boundaries. Arguments (paths,
//! provider responses and document text) are copied verbatim, never translated.
use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Message {
    en: String,
    ru: String,
}
struct Pattern {
    regex: Regex,
    arguments: Vec<String>,
    replacement: String,
}
static ENGLISH: OnceLock<Vec<Pattern>> = OnceLock::new();
static RUSSIAN: OnceLock<Vec<Pattern>> = OnceLock::new();
static PARAMETER: OnceLock<Regex> = OnceLock::new();
fn parameter() -> &'static Regex {
    PARAMETER.get_or_init(|| Regex::new(r"\{([^{}]*)\}").unwrap())
}

fn tokens(value: &str) -> Vec<String> {
    let mut positional = 0;
    parameter()
        .captures_iter(value)
        .map(|capture| {
            let name = capture[1].split(':').next().unwrap();
            if name.is_empty() {
                let token = positional.to_string();
                positional += 1;
                token
            } else {
                name.into()
            }
        })
        .collect()
}

fn patterns(russian: bool) -> Vec<Pattern> {
    let messages: Vec<Message> = serde_json::from_str(include_str!("messages.json"))
        .expect("validated native message catalog");
    messages
        .into_iter()
        .map(|m| {
            let (source, replacement) = if russian { (m.en, m.ru) } else { (m.ru, m.en) };
            let mut expression = String::from("(?s)^");
            let mut end = 0;
            for capture in parameter().find_iter(&source) {
                expression.push_str(&regex::escape(&source[end..capture.start()]));
                expression.push_str("(.*?)");
                end = capture.end();
            }
            expression.push_str(&regex::escape(&source[end..]));
            expression.push('$');
            Pattern {
                regex: Regex::new(&expression).unwrap(),
                arguments: tokens(&source),
                replacement,
            }
        })
        .collect()
}

pub fn text(value: &str, language: crate::preferences::Language) -> String {
    localized(value, language, 0)
}

fn localized(value: &str, language: crate::preferences::Language, depth: u8) -> String {
    // Only known error envelopes may contain another application message.
    // Do not recursively translate path/template arguments or external data.
    if depth < 4 {
        for (en, ru) in [
            (
                "The provider returned an error: ",
                "Провайдер вернул ошибку: ",
            ),
            (
                "Unexpected provider response: ",
                "Неожиданный ответ провайдера: ",
            ),
        ] {
            if let Some(inner) = value.strip_prefix(en).or_else(|| value.strip_prefix(ru)) {
                let prefix = if language == crate::preferences::Language::En {
                    en
                } else {
                    ru
                };
                return format!("{prefix}{}", localized(inner, language, depth + 1));
            }
        }
    }
    let patterns = if language == crate::preferences::Language::Ru {
        RUSSIAN.get_or_init(|| patterns(true))
    } else {
        ENGLISH.get_or_init(|| patterns(false))
    };
    for pattern in patterns {
        if let Some(captures) = pattern.regex.captures(value) {
            let names = tokens(&pattern.replacement);
            let mut index = 0;
            return parameter()
                .replace_all(&pattern.replacement, |_: &regex::Captures<'_>| {
                    let position = pattern
                        .arguments
                        .iter()
                        .position(|name| name == &names[index])
                        .expect("matching parameters");
                    index += 1;
                    captures[position + 1].to_string()
                })
                .into_owned();
        }
    }
    value.into()
}

pub fn error(value: String) -> String {
    text(&value, crate::preferences::language())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::Language;
    #[test]
    fn native_catalog_parameters_match_and_errors_preserve_arguments() {
        let messages: Vec<Message> = serde_json::from_str(include_str!("messages.json")).unwrap();
        for m in messages {
            let (mut en, mut ru) = (tokens(&m.en), tokens(&m.ru));
            en.sort();
            ru.sort();
            assert_eq!(en, ru, "{}", m.en);
        }
        assert_eq!(
            text("Документ не найден", Language::En),
            "Document not found"
        );
        assert_eq!(
            text("Document not found", Language::Ru),
            "Документ не найден"
        );
        assert_eq!(
            text(
                "Папка для резервной копии не существует: C:\\Мои документы",
                Language::En
            ),
            "The backup folder does not exist: C:\\Мои документы"
        );
        assert_eq!(
            text("Не удалось записать модель: {private data}", Language::En),
            "Could not write the model file: {private data}"
        );
        assert_eq!(text("Незнакомый текст", Language::En), "Незнакомый текст");
        assert_eq!(
            text(
                "Провайдер вернул ошибку: The Moonshine worker exited.",
                Language::Ru
            ),
            "Провайдер вернул ошибку: Движок Moonshine завершился."
        );
        assert_eq!(
            text(
                "Manifest заявляет 3 аудиофайлов, в архиве найдено 2.",
                Language::En
            ),
            "The manifest lists 3 audio files, but the archive contains 2."
        );
    }
}
