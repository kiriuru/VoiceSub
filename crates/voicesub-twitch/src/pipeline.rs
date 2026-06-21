use crate::emotes::EmoteRegistry;
use crate::lang::{language_allowed, resolve_message_language};
use crate::replacements::resolve_spoken_nick;
use crate::settings::TwitchTtsSettings;
use crate::source_text_replacement::{
    SourceTextReplacementSettings, apply_builtin_profanity, profanity_settings_for_twitch,
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

    if settings.strip_links {
        clean_text = crate::links::strip_links_from_text(&clean_text);
        clean_text = crate::emoji::normalize_whitespace(&clean_text);
    }

    clean_text = crate::lang::strip_leading_speaker_label(&clean_text);
    clean_text = crate::emoji::strip_invisible_chat_characters(&clean_text);
    clean_text = crate::emoji::normalize_whitespace(&clean_text);

    let tts_text = apply_post_mention_filters(
        &crate::lang::normalize_twitch_mentions(&clean_text),
        settings,
    );
    clean_text = apply_post_mention_filters(
        &crate::lang::strip_twitch_mentions(&clean_text),
        settings,
    );

    if !crate::lang::has_meaningful_linguistic_content(&clean_text, settings.strip_links) {
        return empty_result(
            settings,
            login,
            display_name,
            raw_text,
            false,
            Some("min_chars"),
        );
    }

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

    let spoken_nick = strip_symbols_for_speech(
        &resolve_spoken_nick(settings, login, display_name),
        settings,
    );

    let fallback_lang = settings.language.trim().to_lowercase();
    let language = if settings.detect_language {
        resolve_message_language(
            &clean_text,
            settings.lang_min_chars as usize,
            &fallback_lang,
            settings.strip_links,
        )
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

    let speak_text = build_speak_text(settings, &spoken_nick, &tts_text);
    ProcessedChatMessage {
        clean_text,
        spoken_nick,
        speak_text,
        language,
        speakable: true,
        skip_reason: None,
    }
}

fn apply_post_mention_filters(text: &str, settings: &TwitchTtsSettings) -> String {
    let mut out = crate::symbols::strip_configured_symbols(text, &settings.strip_symbols);
    if settings.replace_underscore_with_space {
        out = crate::symbols::replace_underscores_with_spaces(&out);
    }
    if settings.strip_links {
        out = crate::links::strip_links_from_text(&out);
        out = crate::emoji::normalize_whitespace(&out);
    }
    apply_builtin_profanity(&out, &profanity_settings_for_twitch(settings))
}

fn strip_symbols_for_speech(text: &str, settings: &TwitchTtsSettings) -> String {
    let mut out = crate::symbols::strip_configured_symbols(text, &settings.strip_symbols);
    if settings.replace_underscore_with_space {
        out = crate::symbols::replace_underscores_with_spaces(&out);
    }
    out
}

fn build_speak_text(settings: &TwitchTtsSettings, spoken_nick: &str, clean_text: &str) -> String {
    if !settings.include_username {
        return clean_text.to_string();
    }
    let pause = crate::settings::pause_separator(settings.pause_style);
    let template = crate::settings::normalize_speak_template(settings.speak_template.trim());
    if template.contains("{nick}") || template.contains("{text}") || template.contains("{pause}") {
        return template
            .replace("{pause}", pause)
            .replace("{nick}", spoken_nick)
            .replace("{text}", clean_text);
    }
    format!("{spoken_nick}{pause}{clean_text}")
}

fn empty_result(
    settings: &TwitchTtsSettings,
    login: &str,
    display_name: &str,
    raw_text: &str,
    speakable: bool,
    skip_reason: Option<&'static str>,
) -> ProcessedChatMessage {
    let spoken_nick = strip_symbols_for_speech(
        &resolve_spoken_nick(settings, login, display_name),
        settings,
    );
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
    use crate::source_text_replacement::{
        SourceTextReplacementPair, SourceTextReplacementSettings,
    };

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
    fn pause_style_controls_default_template_separator() {
        let registry = EmoteRegistry::new();
        let period = TwitchTtsSettings::default();
        let comma = TwitchTtsSettings {
            pause_style: crate::settings::TwitchPauseStyle::Comma,
            ..Default::default()
        };
        let period_out = process_chat_message(
            &period,
            &no_replacement(),
            &registry,
            "bob",
            "Bob",
            "hello",
            None,
        );
        let comma_out = process_chat_message(
            &comma,
            &no_replacement(),
            &registry,
            "bob",
            "Bob",
            "hello",
            None,
        );
        assert_eq!(period_out.speak_text, "Bob. hello");
        assert_eq!(comma_out.speak_text, "Bob, hello");
    }

    #[test]
    fn legacy_template_uses_pause_style_not_hardcoded_punctuation() {
        let settings = TwitchTtsSettings {
            speak_template: "{nick}. {text}".into(),
            pause_style: crate::settings::TwitchPauseStyle::Dash,
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "bob",
            "Bob",
            "hello",
            None,
        );
        assert_eq!(out.speak_text, "Bob — hello");
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

    #[test]
    fn strip_links_removes_urls_from_speech() {
        let settings = TwitchTtsSettings::default();
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "u",
            "User",
            "look https://twitch.tv/foo please",
            None,
        );
        assert!(out.speakable);
        assert_eq!(out.clean_text, "look please");
        assert_eq!(out.speak_text, "User. look please");
    }

    #[test]
    fn strip_links_disabled_keeps_urls() {
        let settings = TwitchTtsSettings {
            strip_links: false,
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "u",
            "User",
            "go https://twitch.tv/foo",
            None,
        );
        assert!(out.speakable);
        assert_eq!(out.clean_text, "go https://twitch.tv/foo");
    }

    #[test]
    fn https_url_survives_emote_and_symbol_filters() {
        let settings = TwitchTtsSettings {
            strip_links: false,
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        let raw = "https://github.com/kiriuru/VoiceSub";
        let after_emotes = registry.clean_message_text(
            raw,
            None,
            &settings.emote_sources,
            settings.strip_emoji,
        );
        assert_eq!(after_emotes, raw, "emote pass mangled URL");
        let after_symbols = crate::symbols::strip_configured_symbols(
            &after_emotes,
            &settings.strip_symbols,
        );
        assert_eq!(after_symbols, raw, "symbol pass mangled URL: {after_symbols}");
    }

    #[test]
    fn link_only_github_speakable_when_strip_links_disabled() {
        let settings = TwitchTtsSettings {
            strip_links: false,
            pause_style: crate::settings::TwitchPauseStyle::Comma,
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        for (sample, expected) in [
            ("github.com/kiriuru/VoiceSub", "github.com/kiriuru/VoiceSub"),
            (
                "https://github.com/kiriuru/VoiceSub",
                "https://github.com/kiriuru/VoiceSub",
            ),
            (
                "https://github.com/kiriuru/VoiceSub\u{034F}",
                "https://github.com/kiriuru/VoiceSub",
            ),
        ] {
            let out = process_chat_message(
                &settings,
                &no_replacement(),
                &registry,
                "kiriuru",
                "Kiriuru",
                sample,
                None,
            );
            assert!(
                out.speakable,
                "expected speakable for {sample:?}, got {:?}",
                out.skip_reason
            );
            assert_eq!(out.clean_text, expected);
            assert!(out.speak_text.contains(expected));
        }
    }

    #[test]
    fn mention_reply_detects_russian_not_portuguese() {
        let settings = TwitchTtsSettings::default();
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "sasha_12041998",
            "sasha_12041998",
            "@KamakiriMeido Привет",
            None,
        );
        assert!(out.speakable);
        assert_eq!(out.language, "ru");
        assert_eq!(out.clean_text, "Привет");
        assert_eq!(out.spoken_nick, "sasha 12041998");
        assert_eq!(out.speak_text, "sasha 12041998. KamakiriMeido Привет");
    }

    #[test]
    fn mention_username_is_spoken_but_not_in_clean_text() {
        let settings = TwitchTtsSettings {
            pause_style: crate::settings::TwitchPauseStyle::Comma,
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "kiriuru",
            "Kiriuru",
            "@Kiriuru hello",
            None,
        );
        assert!(out.speakable);
        assert_eq!(out.language, "en");
        assert_eq!(out.clean_text, "hello");
        assert_eq!(out.speak_text, "Kiriuru, Kiriuru hello");
    }

    #[test]
    fn link_only_speaker_label_is_not_speakable() {
        let settings = TwitchTtsSettings::default();
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "wallenber",
            "Wallenber",
            "Wallenber: https://www.youtube.com/watch?v=zqBnOfSmKQo",
            None,
        );
        assert!(!out.speakable);
        assert_eq!(out.skip_reason, Some("min_chars"));
    }

    #[test]
    fn youtube_playlist_link_line_is_not_speakable() {
        let settings = TwitchTtsSettings::default();
        let registry = EmoteRegistry::new();
        let sample = "Wallenber: https://www.youtube.com/watch?v=3VTkBuxU4yk&list=RDMM&index=5";
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "wallenber",
            "Wallenber",
            sample,
            None,
        );
        assert!(!out.speakable);
        assert_eq!(out.skip_reason, Some("min_chars"));
    }

    #[test]
    fn bare_youtube_url_is_speakable_when_strip_links_disabled() {
        let settings = TwitchTtsSettings {
            strip_links: false,
            strip_symbols: vec!["@".into()],
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        let sample = "https://www.youtube.com/watch?v=3VTkBuxU4yk&list=RDMM&index=5";
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "wallenber",
            "Wallenber",
            sample,
            None,
        );
        assert!(out.speakable, "expected speakable, got {:?}", out.skip_reason);
        assert_eq!(out.clean_text, sample);
        assert!(out.speak_text.contains("youtube.com"));
    }

    #[test]
    fn broken_url_after_symbol_strip_does_not_detect_dutch() {
        let settings = TwitchTtsSettings {
            strip_links: false,
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        let sample = "https://www.youtube.com/watch?v=3VTkBuxU4yk&list=RDMM&index=5";
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "wallenber",
            "Wallenber",
            sample,
            None,
        );
        assert!(out.speakable);
        assert_ne!(out.language, "nl");
    }

    #[test]
    fn strip_symbols_removes_configured_tokens_from_speech() {
        let settings = TwitchTtsSettings {
            strip_symbols: vec!["@".into(), "&".into(), "$".into()],
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "u",
            "User",
            "pay & go @all",
            None,
        );
        assert!(out.speakable);
        assert_eq!(out.clean_text, "pay go");
        assert_eq!(out.speak_text, "User. pay go all");
    }

    #[test]
    fn digit_separators_keep_number_groups() {
        let settings = TwitchTtsSettings::default();
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "kiriuru",
            "Kiriuru",
            "500&100",
            None,
        );
        assert!(out.speakable);
        assert_eq!(out.clean_text, "500 100");
        assert_eq!(out.speak_text, "Kiriuru. 500 100");
    }

    #[test]
    fn replace_underscore_with_space_in_message_and_nick() {
        let settings = TwitchTtsSettings {
            replace_underscore_with_space: true,
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "cool_guy",
            "Cool_Guy",
            "see you_later",
            None,
        );
        assert!(out.speakable);
        assert_eq!(out.clean_text, "see you later");
        assert_eq!(out.spoken_nick, "Cool Guy");
        assert_eq!(out.speak_text, "Cool Guy. see you later");
    }

    #[test]
    fn strip_symbols_removes_underscore_from_message_and_nick() {
        let settings = TwitchTtsSettings {
            replace_underscore_with_space: false,
            strip_symbols: vec!["@".into(), "&".into(), "$".into(), "_".into()],
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "cool_guy",
            "Cool_Guy",
            "see you_later",
            None,
        );
        assert!(out.speakable);
        assert_eq!(out.clean_text, "see youlater");
        assert_eq!(out.spoken_nick, "CoolGuy");
        assert_eq!(out.speak_text, "CoolGuy. see youlater");
    }

    #[test]
    fn empty_strip_symbols_keeps_special_chars() {
        let settings = TwitchTtsSettings {
            strip_symbols: vec![],
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "u",
            "User",
            "a & b",
            None,
        );
        assert!(out.speakable);
        assert_eq!(out.clean_text, "a & b");
    }

    #[test]
    fn preserves_digits_in_russian_chat_line() {
        let settings = TwitchTtsSettings::default();
        let registry = EmoteRegistry::new();
        let sample = "я ограничился 5ю каналами, но по идее можно до 100 сделать";
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "kiriuru",
            "Kiriuru",
            sample,
            None,
        );
        assert!(out.speakable, "expected speakable: {:?}", out.skip_reason);
        assert!(
            out.clean_text.contains('5') && out.clean_text.contains("100"),
            "digits must remain in clean_text, got: {}",
            out.clean_text
        );
    }

    #[test]
    fn digit_only_with_separators_is_speakable() {
        let settings = TwitchTtsSettings::default();
        let registry = EmoteRegistry::new();
        for sample in ["500&100", "500$100"] {
            let out = process_chat_message(
                &settings,
                &no_replacement(),
                &registry,
                "kiriuru",
                "Kiriuru",
                sample,
                None,
            );
            assert!(
                out.speakable,
                "expected speakable for {sample:?}, got {:?}",
                out.skip_reason
            );
            assert_eq!(out.clean_text, "500 100");
            assert!(out.speak_text.contains("500 100"));
        }
    }

    #[test]
    fn digit_separators_speakable_with_at_only_symbol_strip() {
        let settings = TwitchTtsSettings {
            strip_symbols: vec!["@".into()],
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        for sample in ["500&100", "500$100", "500/100"] {
            let out = process_chat_message(
                &settings,
                &no_replacement(),
                &registry,
                "kiriuru",
                "Kiriuru",
                sample,
                None,
            );
            assert!(
                out.speakable,
                "expected speakable for {sample:?}, got {:?}",
                out.skip_reason
            );
            assert_eq!(out.clean_text, sample);
            assert!(out.speak_text.contains(sample));
        }
    }

    #[test]
    fn digit_separators_speakable_with_trailing_invisible_chars() {
        let settings = TwitchTtsSettings {
            strip_symbols: vec!["@".into()],
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        for (sample, expected) in [
            ("500&100\u{034F}", "500&100"),
            ("500&100 \u{3164}", "500&100"),
            ("500$100\u{200B}", "500$100"),
        ] {
            let out = process_chat_message(
                &settings,
                &no_replacement(),
                &registry,
                "kiriuru",
                "Kiriuru",
                sample,
                None,
            );
            assert!(
                out.speakable,
                "expected speakable for {sample:?}, got {:?}",
                out.skip_reason
            );
            assert_eq!(out.clean_text, expected);
            assert!(out.speak_text.contains(expected));
        }
    }

    #[test]
    fn digit_separators_speakable_even_without_symbol_strip() {
        let settings = TwitchTtsSettings {
            strip_symbols: vec![],
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        for sample in ["500&100", "500$100"] {
            let out = process_chat_message(
                &settings,
                &no_replacement(),
                &registry,
                "kiriuru",
                "Kiriuru",
                sample,
                None,
            );
            assert!(
                out.speakable,
                "expected speakable for {sample:?}, got {:?}",
                out.skip_reason
            );
            assert_eq!(out.clean_text, sample);
            assert!(out.speak_text.contains(sample));
        }
    }

    #[test]
    fn digit_only_chat_lines_are_speakable() {
        let settings = TwitchTtsSettings::default();
        let registry = EmoteRegistry::new();
        for sample in ["522", "123", "1 2 3"] {
            let out = process_chat_message(
                &settings,
                &no_replacement(),
                &registry,
                "kiriuru",
                "Kiriuru",
                sample,
                None,
            );
            assert!(
                out.speakable,
                "expected speakable for {sample:?}, got {:?}",
                out.skip_reason
            );
            assert_eq!(out.clean_text, sample);
            assert!(out.speak_text.contains(sample));
        }
    }

    #[test]
    fn pipeline_strips_bttv_emotes_from_speech() {
        let settings = TwitchTtsSettings::default();
        let registry = EmoteRegistry::new();
        registry.seed_test_emotes(&[], &["OMEGALUL", "NOPERS"]);
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "viewer",
            "Viewer",
            "OMEGALUL NOPERS gg",
            None,
        );
        assert!(out.speakable);
        assert_eq!(out.clean_text, "gg");
        assert_eq!(out.speak_text, "Viewer. gg");
    }

    #[test]
    fn pipeline_strips_seventv_emotes_from_speech() {
        let settings = TwitchTtsSettings::default();
        let registry = EmoteRegistry::new();
        registry.seed_test_emotes_with_seventv(&[], &[], &["RainbowPls", "Clap"]);
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "viewer",
            "Viewer",
            "RainbowPls Clap nice",
            None,
        );
        assert!(out.speakable);
        assert_eq!(out.clean_text, "nice");
        assert_eq!(out.speak_text, "Viewer. nice");
    }

    #[test]
    fn pipeline_strips_emotes_but_preserves_digits() {
        let settings = TwitchTtsSettings::default();
        let registry = EmoteRegistry::new();
        registry.seed_test_emotes(&["kappa"], &["OMEGALUL"]);
        let out = process_chat_message(
            &settings,
            &no_replacement(),
            &registry,
            "u",
            "User",
            "Kappa OMEGALUL нужно 100 и 5ю каналов",
            None,
        );
        assert!(out.speakable);
        assert_eq!(out.clean_text, "нужно 100 и 5ю каналов");
        assert!(out.speak_text.contains("100"));
        assert!(out.speak_text.contains('5'));
    }
}
