//! Configurable symbol stripping for Twitch chat TTS.

/// Replace underscore characters with spaces before TTS.
pub fn replace_underscores_with_spaces(text: &str) -> String {
    crate::emoji::normalize_whitespace(&text.replace('_', " "))
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
    entries.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()));

    let mut result = text.to_string();
    for symbol in entries {
        if symbol.is_empty() {
            continue;
        }
        result = result.replace(symbol.as_str(), "");
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
    fn supports_multi_char_tokens() {
        assert_eq!(
            strip_configured_symbols("wait... ok", &["...".into()]),
            "wait ok"
        );
    }
}
