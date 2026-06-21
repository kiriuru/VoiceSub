//! Configurable symbol stripping for Twitch chat TTS.

use crate::emoji::is_plain_decimal_char;

/// Replace underscore characters with spaces before TTS.
pub fn replace_underscores_with_spaces(text: &str) -> String {
    crate::emoji::normalize_whitespace(&text.replace('_', " "))
}

fn prev_significant_char(chars: &[char], index: usize) -> Option<char> {
    chars[..index]
        .iter()
        .rev()
        .find(|ch| !ch.is_whitespace())
        .copied()
}

fn next_significant_char(chars: &[char], index: usize) -> Option<char> {
    chars[index + 1..]
        .iter()
        .find(|ch| !ch.is_whitespace())
        .copied()
}

fn strip_single_char_symbol(text: &str, symbol: char) -> String {
    if symbol == '_' {
        return text.replace(symbol, "");
    }

    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (index, ch) in chars.iter().enumerate() {
        if *ch == symbol {
            let between_digits = matches!(
                (prev_significant_char(&chars, index), next_significant_char(&chars, index)),
                (Some(prev), Some(next))
                    if is_plain_decimal_char(prev) && is_plain_decimal_char(next)
            );
            if between_digits {
                out.push(' ');
            }
            continue;
        }
        out.push(*ch);
    }
    out
}

/// Remove configured symbol tokens from chat text before TTS.
pub fn strip_configured_symbols(text: &str, symbols: &[String]) -> String {
    let mut entries: Vec<String> = symbols
        .iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect();
    if entries.is_empty() {
        return text.to_string();
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.chars().count()));

    let mut result = text.to_string();
    for symbol in entries {
        if symbol.is_empty() {
            continue;
        }
        if symbol.chars().count() == 1 {
            let ch = symbol.chars().next().expect("non-empty single char");
            result = strip_single_char_symbol(&result, ch);
        } else {
            result = result.replace(symbol.as_str(), " ");
        }
    }
    crate::emoji::normalize_whitespace(&result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_list_keeps_symbols() {
        assert_eq!(strip_configured_symbols("a @ b & c", &[]), "a @ b & c");
    }

    #[test]
    fn strips_listed_single_characters() {
        let symbols = vec!["@".into(), "&".into(), "$".into()];
        assert_eq!(
            strip_configured_symbols("hi @you & me $100", &symbols),
            "hi you me 100"
        );
    }

    #[test]
    fn replaces_underscore_with_space() {
        assert_eq!(
            replace_underscores_with_spaces("cool_guy see you_later"),
            "cool guy see you later"
        );
    }

    #[test]
    fn strips_underscore_token_when_configured() {
        let symbols = vec!["@".into(), "&".into(), "$".into(), "_".into()];
        assert_eq!(
            strip_configured_symbols("snake_case @you & me $1", &symbols),
            "snakecase you me 1"
        );
    }

    #[test]
    fn separator_symbols_keep_space_between_digit_groups() {
        let symbols = vec!["@".into(), "&".into(), "$".into()];
        assert_eq!(strip_configured_symbols("500&100", &symbols), "500 100");
        assert_eq!(strip_configured_symbols("500$100", &symbols), "500 100");
    }

    #[test]
    fn url_ampersands_stay_glued_for_link_detection() {
        let symbols = vec!["@".into(), "&".into(), "$".into()];
        let sample = "https://www.youtube.com/watch?v=3VTkBuxU4yk&list=RDMM&index=5";
        assert_eq!(
            strip_configured_symbols(sample, &symbols),
            "https://www.youtube.com/watch?v=3VTkBuxU4yklist=RDMMindex=5"
        );
    }

    #[test]
    fn supports_multi_char_tokens() {
        assert_eq!(
            strip_configured_symbols("wait... ok", &["...".into()]),
            "wait ok"
        );
    }
}
