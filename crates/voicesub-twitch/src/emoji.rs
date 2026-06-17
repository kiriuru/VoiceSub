use std::sync::OnceLock;

use regex::Regex;

fn emoji_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\p{Emoji}").expect("emoji regex"))
}

/// Decimal digits used as numbers in chat — must not be stripped as `\p{Emoji}` (ASCII 0–9
/// are Emoji components for keycap sequences) or as third-party emote codes.
pub fn is_plain_decimal_char(ch: char) -> bool {
    ch.is_ascii_digit()
        || matches!(
            ch,
            '\u{0660}'..='\u{0669}' // Arabic-Indic
                | '\u{06F0}'..='\u{06F9}' // Extended Arabic-Indic
                | '\u{FF10}'..='\u{FF19}' // Fullwidth
        )
}

pub fn is_plain_decimal_token(token: &str) -> bool {
    let trimmed = token.trim_matches(|ch: char| {
        matches!(
            ch,
            ',' | '.' | '!' | '?' | ';' | ':' | ')' | ']' | '"' | '\''
        )
    });
    !trimmed.is_empty() && trimmed.chars().all(is_plain_decimal_char)
}

fn preserve_digit_emoji_match(matched: &str) -> String {
    if matched.is_empty() {
        return String::new();
    }
    if matched.chars().all(is_plain_decimal_char) {
        return matched.to_string();
    }
    let digits: String = matched
        .chars()
        .filter(|ch| is_plain_decimal_char(*ch))
        .collect();
    digits
}

pub fn strip_unicode_emoji(text: &str) -> String {
    let stripped = emoji_regex().replace_all(text, |caps: &regex::Captures<'_>| {
        let matched = caps.get(0).map(|m| m.as_str()).unwrap_or("");
        preserve_digit_emoji_match(matched)
    });
    let mut rest = stripped.as_ref();
    let mut out = String::with_capacity(rest.len());
    while !rest.is_empty() {
        if let Some(emoji) = emojis::get(rest) {
            let slice = emoji.as_str();
            let preserved = preserve_digit_emoji_match(slice);
            if !preserved.is_empty() {
                out.push_str(&preserved);
            }
            rest = &rest[slice.len()..];
            continue;
        }
        let ch_len = rest
            .chars()
            .next()
            .map(|ch| ch.len_utf8())
            .unwrap_or(rest.len());
        out.push_str(&rest[..ch_len]);
        rest = &rest[ch_len..];
    }
    out
}

pub fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_common_emoji() {
        let input = "hello \u{1F600} world";
        let out = strip_unicode_emoji(input);
        assert_eq!(normalize_whitespace(&out), "hello world");
    }

    #[test]
    fn collapses_whitespace() {
        assert_eq!(normalize_whitespace("  a   b  "), "a b");
    }

    #[test]
    fn preserves_ascii_digits_used_as_numbers() {
        let input = "я ограничился 5ю каналами, но по идее можно до 100 сделать";
        assert_eq!(strip_unicode_emoji(input), input);
    }

    #[test]
    fn preserves_all_ascii_digit_runs() {
        for digit in '0'..='9' {
            let sample = format!("x {digit} y {digit}{digit}{digit} z");
            assert_eq!(strip_unicode_emoji(&sample), sample);
        }
        assert_eq!(
            strip_unicode_emoji("0 42 999 1234567890"),
            "0 42 999 1234567890"
        );
    }

    #[test]
    fn preserves_unicode_decimal_digit_blocks() {
        assert_eq!(
            strip_unicode_emoji("count \u{0661}\u{0662}\u{0663} ok"),
            "count \u{0661}\u{0662}\u{0663} ok"
        );
        assert_eq!(
            strip_unicode_emoji("count \u{06F4}\u{06F5} ok"),
            "count \u{06F4}\u{06F5} ok"
        );
        assert_eq!(
            strip_unicode_emoji("count \u{FF11}\u{FF12} ok"),
            "count \u{FF11}\u{FF12} ok"
        );
    }

    #[test]
    fn strips_face_emoji_but_keeps_adjacent_digits() {
        let input = "gg \u{1F600} 42 wins";
        assert_eq!(strip_unicode_emoji(input), "gg  42 wins");
    }

    #[test]
    fn keycap_emoji_sequence_keeps_base_digit() {
        let keycap = "5\u{FE0F}\u{20E3}";
        assert_eq!(strip_unicode_emoji(keycap), "5");
    }

    #[test]
    fn plain_decimal_token_helper() {
        assert!(is_plain_decimal_token("100"));
        assert!(is_plain_decimal_token("100,"));
        assert!(is_plain_decimal_token("42!"));
        assert!(!is_plain_decimal_token("5ю"));
        assert!(!is_plain_decimal_token("Kappa"));
    }
}
