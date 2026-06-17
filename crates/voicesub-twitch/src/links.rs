//! URL token detection and removal for Twitch chat TTS.

const TRAILING_PUNCT: &[char] = &['.', ',', '!', '?', ';', ')', ']', '"', '\''];

const URL_HOST_MARKERS: &[&str] = &[
    "youtube.com/",
    "youtu.be/",
    "twitch.tv/",
    "discord.gg/",
    "discord.com/",
    "x.com/",
    "twitter.com/",
    "kick.com/",
    "instagram.com/",
    "tiktok.com/",
    "spotify.com/",
    "open.spotify.com/",
];

fn trim_trailing_punctuation(token: &str) -> &str {
    token.trim_end_matches(TRAILING_PUNCT)
}

/// Returns true when a whitespace-delimited token looks like a hyperlink or host/path link.
pub fn looks_like_url_token(token: &str) -> bool {
    let token = trim_trailing_punctuation(token.trim());
    if token.is_empty() {
        return false;
    }
    let lower = token.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return true;
    }
    if lower.starts_with("www.") {
        return true;
    }
    if lower.contains("://") {
        return true;
    }
    if lower.contains("watch?v=") || lower.contains("watch?si=") {
        return true;
    }
    if URL_HOST_MARKERS.iter().any(|marker| lower.contains(marker)) {
        return true;
    }
    // host.tld/path (discord.gg/invite, twitch.tv/name, …)
    if let Some(slash_idx) = lower.find('/') {
        if slash_idx == 0 {
            return false;
        }
        let host = lower[..slash_idx].split('@').next_back().unwrap_or("");
        if host.contains('.') && host.len() >= 4 {
            return true;
        }
    }
    false
}

fn strip_url_tokens(text: &str) -> String {
    text.split_whitespace()
        .filter(|token| !looks_like_url_token(token))
        .collect::<Vec<_>>()
        .join(" ")
}

fn consume_non_whitespace_run(text: &str, start: usize) -> usize {
    let mut i = start;
    for ch in text[start..].chars() {
        if ch.is_whitespace() {
            break;
        }
        i += ch.len_utf8();
    }
    i - start
}

/// Remove inline `http(s)://` and `www.` spans even when glued to punctuation (`Name:https://…`).
fn strip_inline_schemes(text: &str) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let lower = rest.to_ascii_lowercase();
        if lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("www.")
        {
            i += consume_non_whitespace_run(text, i);
            continue;
        }
        let ch = rest.chars().next().expect("char at i");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Remove URL-like tokens; keeps surrounding message text when possible.
pub fn strip_links_from_text(text: &str) -> String {
    let pass1 = strip_inline_schemes(text);
    let pass2 = strip_url_tokens(&pass1);
    crate::emoji::normalize_whitespace(&pass2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_http_and_https() {
        assert!(looks_like_url_token("https://twitch.tv/foo"));
        assert!(looks_like_url_token("http://example.com"));
    }

    #[test]
    fn detects_www_and_host_path() {
        assert!(looks_like_url_token("www.youtube.com/watch?v=1"));
        assert!(looks_like_url_token("discord.gg/invite"));
    }

    #[test]
    fn detects_youtube_query_urls() {
        let url = "https://www.youtube.com/watch?v=3VTkBuxU4yk&list=RDMM&index=5";
        assert!(looks_like_url_token(url));
        assert_eq!(strip_links_from_text(url), "");
    }

    #[test]
    fn detects_broken_url_after_ampersand_strip() {
        let broken = "https://www.youtube.com/watch?v=3VTkBuxU4yklist=RDMMindex=5";
        assert!(looks_like_url_token(broken));
    }

    #[test]
    fn detects_watch_query_fragments() {
        assert!(looks_like_url_token("watch?v=VTkBuxUyklist=RDMMindex=5"));
    }

    #[test]
    fn ignores_plain_words() {
        assert!(!looks_like_url_token("hello"));
        assert!(!looks_like_url_token("ok.ru")); // no path — keep short host-like nicknames
    }

    #[test]
    fn strips_links_but_keeps_message() {
        assert_eq!(
            strip_links_from_text("check this https://twitch.tv/x out"),
            "check this out"
        );
    }

    #[test]
    fn strips_speaker_glued_to_scheme() {
        assert_eq!(
            strip_links_from_text("Wallenber:https://www.youtube.com/watch?v=1"),
            "Wallenber:"
        );
    }

    #[test]
    fn strips_wallenber_youtube_line() {
        let sample = "Wallenber: https://www.youtube.com/watch?v=3VTkBuxU4yk&list=RDMM&index=5";
        assert_eq!(strip_links_from_text(sample), "Wallenber:");
    }

    #[test]
    fn trims_trailing_punctuation_on_urls() {
        assert!(looks_like_url_token("https://twitch.tv/x,"));
        assert_eq!(strip_links_from_text("look: https://twitch.tv/x."), "look:");
    }
}
