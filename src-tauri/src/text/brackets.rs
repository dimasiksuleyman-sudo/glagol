//! «Не читать текст в скобках»: remove parenthesized and bracketed asides
//! before synthesis (an opt-in Text to speech setting).

/// Remove `(…)` and `[…]` with their contents, including nested ones, and
/// tidy the spaces left behind. Unbalanced brackets are kept as they are.
pub fn strip(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '(' || c == '[' {
            if let Some(end) = closing(&chars, i) {
                // Drop the space before the aside: «слово (пояснение).» → «слово.»
                while out.ends_with(' ') {
                    out.pop();
                }
                i = end + 1;
                let next = chars.get(i).copied();
                if !out.is_empty()
                    && next.is_some_and(|n| n.is_alphanumeric() || n == '(' || n == '[')
                    && !out.ends_with(['\n', '«', '"'])
                {
                    out.push(' ');
                }
                while chars.get(i) == Some(&' ') && (out.is_empty() || out.ends_with(' ')) {
                    i += 1;
                }
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Index of the bracket closing the one at `start` (same paragraph), if any.
fn closing(chars: &[char], start: usize) -> Option<usize> {
    let mut stack = vec![chars[start]];
    for (offset, &c) in chars[start + 1..].iter().enumerate() {
        match c {
            '(' | '[' => stack.push(c),
            ')' | ']' => {
                let open = stack.pop()?;
                if (open == '(') != (c == ')') {
                    return None;
                }
                if stack.is_empty() {
                    return Some(start + 1 + offset);
                }
            }
            '\n' => return None,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::strip;

    #[test]
    fn asides_are_removed_and_spaces_tidied() {
        assert_eq!(
            strip("Компания (основана в 1998 году) выросла."),
            "Компания выросла."
        );
        assert_eq!(strip("Итог (см. табл. 2)."), "Итог.");
        assert_eq!(strip("Ссылка [1] и [2, 3] тут."), "Ссылка и тут.");
        assert_eq!(strip("Вложенные (а (б) в) скобки."), "Вложенные скобки.");
        assert_eq!(strip("(Примечание.) Текст."), "Текст.");
    }

    #[test]
    fn unbalanced_or_multiline_brackets_stay() {
        assert_eq!(strip("Смайлик :) остаётся."), "Смайлик :) остаётся.");
        assert_eq!(strip("Открыта ( и не закрыта."), "Открыта ( и не закрыта.");
        assert_eq!(strip("Начало (\nконец)."), "Начало (\nконец).");
        assert_eq!(strip("Без скобок."), "Без скобок.");
    }
}
