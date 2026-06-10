use crate::emotes::EmoteRegistry;
use crate::lang::{language_allowed, resolve_message_language};
use crate::replacements::resolve_spoken_nick;
use crate::settings::{TwitchPauseStyle, TwitchTtsSettings};
use crate::source_text_replacement::{
    apply_builtin_profanity, profanity_settings_for_twitch, SourceTextReplacementSettings,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedChatMessage {
    pub clean_text: String,
    pub spoken_nick: String,
    pub speak_text: String,
    pub language: String,
    pub speakable: bool,
    pub skip_reason: Option<&'static str>,
}

pub fn process_chat_message(
    settings: &TwitchTtsSettings,
    _source_replacement: &SourceTextReplacementSettings,
    emotes: &EmoteRegistry,
    login: &str,
    display_name: &str,
    raw_text: &str,
    irc_emotes_tag: Option<&str>,
) -> ProcessedChatMessage {
    if let Some(reason) = crate::filter::filter_skip_reason(settings, login, raw_text) {
        return empty_result(settings, login, display_name, raw_text, false, Some(reason));
    }

    let mut clean_text = if settings.strip_emotes {
        emotes.clean_message_text(
            raw_text,
            irc_emotes_tag,
            &settings.emote_sources,
            settings.strip_emoji,
        )
    } else if settings.strip_emoji {
        crate::emoji::normalize_whitespace(&crate::emoji::strip_unicode_emoji(raw_text))
    } else {
        raw_text.trim().to_string()
    };

    clean_text = apply_builtin_profanity(&clean_text, &profanity_settings_for_twitch(settings));

    if clean_text.chars().count() < settings.min_chars as usize {
        return empty_result(
            settings,
            login,
            display_name,
            raw_text,
            false,
            Some("min_chars"),
        );
    }

    let max = settings.max_chars.max(1) as usize;
    if clean_text.chars().count() > max {
        return empty_result(
            settings,
            login,
            display_name,
            raw_text,
            false,
            Some("max_chars"),
        );
    }

    let spoken_nick = resolve_spoken_nick(settings, login, display_name);

    let fallback_lang = settings.language.trim().to_lowercase();
    let language = if settings.detect_language {
        resolve_message_language(&clean_text, settings.lang_min_chars as usize, &fallback_lang)
    } else {
        fallback_lang.clone()
    };

    if settings.detect_language && !language_allowed(&language, &settings.enabled_languages) {
        return ProcessedChatMessage {
            clean_text,
            spoken_nick,
            speak_text: String::new(),
            language,
            speakable: false,
            skip_reason: Some("language_filter"),
        };
    }

    let speak_text = build_speak_text(settings, &spoken_nick, &clean_text);
    ProcessedChatMessage {
        clean_text,
        spoken_nick,
        speak_text,
        language,
        speakable: true,
        skip_reason: None,
    }
}

fn build_speak_text(settings: &TwitchTtsSettings, spoken_nick: &str, clean_text: &str) -> String {
    if !settings.include_username {
        return clean_text.to_string();
    }
    let template = settings.speak_template.trim();
    if template.contains("{nick}") || template.contains("{text}") {
        return template
            .replace("{nick}", spoken_nick)
            .replace("{text}", clean_text);
    }
    match settings.pause_style {
        TwitchPauseStyle::Comma => format!("{spoken_nick}, {clean_text}"),
        TwitchPauseStyle::Period => format!("{spoken_nick}. {clean_text}"),
        TwitchPauseStyle::Dash => format!("{spoken_nick} — {clean_text}"),
        TwitchPauseStyle::Ellipsis => format!("{spoken_nick}… {clean_text}"),
    }
}

fn empty_result(
    settings: &TwitchTtsSettings,
    login: &str,
    display_name: &str,
    raw_text: &str,
    speakable: bool,
    skip_reason: Option<&'static str>,
) -> ProcessedChatMessage {
    let spoken_nick = resolve_spoken_nick(settings, login, display_name);
    ProcessedChatMessage {
        clean_text: raw_text.trim().to_string(),
        spoken_nick: spoken_nick.clone(),
        speak_text: if speakable {
            build_speak_text(settings, &spoken_nick, raw_text.trim())
        } else {
            String::new()
        },
        language: settings.language.trim().to_lowercase(),
        speakable,
        skip_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emotes::EmoteRegistry;
    use crate::settings::TwitchTtsSettings;
    use crate::source_text_replacement::{SourceTextReplacementPair, SourceTextReplacementSettings};

    fn no_replacement() -> SourceTextReplacementSettings {
        SourceTextReplacementSettings::default()
    }

    #[test]
    fn builds_period_pause_by_default() {
        let settings = TwitchTtsSettings::default();
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "alice",
            "Alice",
            "hello there",
            None,
        );
        assert!(out.speakable);
        assert_eq!(out.speak_text, "Alice. hello there");
    }

    #[test]
    fn respects_custom_template() {
        let settings = TwitchTtsSettings {
            speak_template: "{nick} says: {text}".into(),
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "bob",
            "Bob",
            "ping",
            None,
        );
        assert_eq!(out.speak_text, "Bob says: ping");
    }

    #[test]
    fn language_filter_blocks_message() {
        let settings = TwitchTtsSettings {
            enabled_languages: vec!["en".into()],
            language: "en".into(),
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "u",
            "U",
            "привет всем друзья",
            None,
        );
        assert!(!out.speakable);
        assert_eq!(out.skip_reason, Some("language_filter"));
    }

    #[test]
    fn max_chars_blocks_long_message() {
        let settings = TwitchTtsSettings {
            max_chars: 10,
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "u",
            "User",
            "this message is definitely too long",
            None,
        );
        assert!(!out.speakable);
        assert_eq!(out.skip_reason, Some("max_chars"));
    }

    #[test]
    fn hello_is_english_not_und() {
        let settings = TwitchTtsSettings::default();
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "kiriuru",
            "Kiriuru",
            "hello",
            None,
        );
        assert!(out.speakable);
        assert_eq!(out.language, "en");
    }

    #[test]
    fn builtin_profanity_uses_twitch_flag_only() {
        let settings = TwitchTtsSettings {
            include_builtin_profanity: true,
            ..Default::default()
        };
        let profanity = SourceTextReplacementSettings {
            enabled: false,
            include_builtin: true,
            case_insensitive: true,
            whole_words: true,
            pairs: vec![SourceTextReplacementPair {
                source: "hello".into(),
                target: "bye".into(),
            }],
        };
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &profanity,
            &registry,
            "u",
            "User",
            "what the fuck",
            None,
        );
        assert_eq!(out.clean_text, "what the ***");
    }

    #[test]
    fn irc_emote_tag_removes_emote_only_message() {
        let settings = TwitchTtsSettings::default();
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "u",
            "User",
            "baleGIGA",
            Some("25:0-7"),
        );
        assert!(!out.speakable);
        assert_eq!(out.skip_reason, Some("min_chars"));
    }
}
