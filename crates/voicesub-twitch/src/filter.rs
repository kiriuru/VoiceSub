use crate::emotes::EmoteRegistry;
use crate::pipeline::process_chat_message;
use crate::settings::{TwitchChatMessage, TwitchTtsSettings};
use crate::source_text_replacement::SourceTextReplacementSettings;

pub fn filter_skip_reason(
    settings: &TwitchTtsSettings,
    user: &str,
    text: &str,
) -> Option<&'static str> {
    if !settings.speak_chat {
        return Some("speak_chat_disabled");
    }
    if settings.block_commands && text.trim().starts_with('!') {
        return Some("block_commands");
    }
    let user_key = user.trim().to_ascii_lowercase();
    if user_key.is_empty() {
        return Some("empty_user");
    }
    if settings
        .ignore_users
        .iter()
        .any(|entry| entry.trim().eq_ignore_ascii_case(&user_key))
    {
        return Some("ignore_list");
    }
    None
}

pub fn should_speak_message(settings: &TwitchTtsSettings, user: &str, text: &str) -> bool {
    filter_skip_reason(settings, user, text).is_none()
}

pub struct ChatMessageInput<'a> {
    pub id: &'a str,
    pub user: &'a str,
    pub display_name: &'a str,
    pub text: &'a str,
    pub channel: &'a str,
    pub is_mod: bool,
    pub is_subscriber: bool,
    pub irc_emotes_tag: Option<&'a str>,
}

pub fn to_chat_message(
    settings: &TwitchTtsSettings,
    source_replacement: &SourceTextReplacementSettings,
    emotes: &EmoteRegistry,
    input: ChatMessageInput<'_>,
) -> TwitchChatMessage {
    let processed = process_chat_message(
        settings,
        source_replacement,
        emotes,
        input.user,
        input.display_name,
        input.text,
        input.irc_emotes_tag,
    );
    TwitchChatMessage {
        id: input.id.to_string(),
        user: input.user.to_string(),
        display_name: input.display_name.to_string(),
        text: input.text.trim().to_string(),
        speak_text: processed.speak_text,
        clean_text: processed.clean_text,
        spoken_nick: processed.spoken_nick,
        channel: input.channel.to_string(),
        language: processed.language,
        is_mod: input.is_mod,
        is_subscriber: input.is_subscriber,
        speakable: processed.speakable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emotes::EmoteRegistry;

    #[test]
    fn blocks_commands() {
        let settings = TwitchTtsSettings::default();
        assert!(!should_speak_message(&settings, "user", "!play"));
    }

    #[test]
    fn respects_ignore_list() {
        let settings = TwitchTtsSettings {
            ignore_users: vec!["bot".into()],
            ..Default::default()
        };
        let registry = EmoteRegistry::new();
        let msg = to_chat_message(
            &settings,
            &Default::default(),
            &registry,
            ChatMessageInput {
                id: "1",
                user: "Bot",
                display_name: "Bot",
                text: "hello there",
                channel: "#ch",
                is_mod: false,
                is_subscriber: false,
                irc_emotes_tag: None,
            },
        );
        assert!(!msg.speakable);
    }
}
