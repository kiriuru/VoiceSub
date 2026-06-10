use std::sync::OnceLock;

use regex::Regex;

fn emoji_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\p{Emoji}").expect("emoji regex"))
}

pub fn strip_unicode_emoji(text: &str) -> String {
    let stripped = emoji_regex().replace_all(text, "");
    let mut rest = stripped.as_ref();
    let mut out = String::with_capacity(rest.len());
    while !rest.is_empty() {
        if let Some(emoji) = emojis::get(rest) {
            rest = &rest[emoji.as_str().len()..];
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
}
