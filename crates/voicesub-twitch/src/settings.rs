use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwitchReplacement {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TwitchEmoteSources {
    pub twitch: bool,
    pub bttv: bool,
    pub seventv: bool,
}

impl Default for TwitchEmoteSources {
    fn default() -> Self {
        Self {
            twitch: true,
            bttv: true,
            seventv: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TwitchPauseStyle {
    Comma,
    #[default]
    Period,
    Dash,
    Ellipsis,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TwitchTtsSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub nick: String,
    #[serde(default)]
    pub oauth_token: String,
    /// Twitch Developer Console Client ID for implicit OAuth (localhost redirect).
    #[serde(default)]
    pub oauth_client_id: String,
    #[serde(default = "default_speak_chat")]
    pub speak_chat: bool,
    #[serde(default = "default_include_username")]
    pub include_username: bool,
    /// Fallback TTS language when auto-detect is off or inconclusive.
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_min_chars")]
    pub min_chars: u32,
    #[serde(default = "default_max_chars")]
    pub max_chars: u32,
    #[serde(default = "default_block_commands")]
    pub block_commands: bool,
    #[serde(default)]
    pub ignore_users: Vec<String>,
    /// Remove Twitch / BTTV / 7TV emote codes from message text.
    #[serde(default = "default_true")]
    pub strip_emotes: bool,
    /// Remove Unicode emoji from message text.
    #[serde(default = "default_true")]
    pub strip_emoji: bool,
    #[serde(default)]
    pub emote_sources: TwitchEmoteSources,
    /// Detect message language (whatlang) for TTS voice selection and filtering.
    #[serde(default = "default_true")]
    pub detect_language: bool,
    /// Minimum cleaned message length for language detection (chars).
    #[serde(default = "default_lang_min_chars")]
    pub lang_min_chars: u32,
    /// Allowed ISO 639-1 codes (`ru`, `en`, `ja`, …). Empty = allow all.
    #[serde(default)]
    pub enabled_languages: Vec<String>,
    #[serde(default)]
    pub nick_replacements: Vec<TwitchReplacement>,
    /// Builtin profanity list for Twitch chat (independent from main-app ASR replacement).
    #[serde(default = "default_true")]
    pub include_builtin_profanity: bool,
    #[serde(default)]
    pub pause_style: TwitchPauseStyle,
    /// TTS template when `include_username` is true. Placeholders: `{nick}`, `{text}`.
    #[serde(default = "default_speak_template")]
    pub speak_template: String,
    /// WASAPI / cpal output label for Twitch chat TTS (empty = system default).
    #[serde(default)]
    pub audio_output_device_id: String,
    #[serde(default)]
    pub audio_output_device_label: String,
    /// Override root `speech_rate` when set (> 0).
    #[serde(default)]
    pub speech_rate: f32,
    /// Override root `speech_volume` when >= 0; `-1` = inherit module default.
    #[serde(default = "default_inherit_volume")]
    pub speech_volume: f32,
    /// Twitch chat queue cap; 0 = use module default (6).
    #[serde(default)]
    pub max_queue_items: u32,
}

fn default_speak_chat() -> bool {
    true
}

fn default_include_username() -> bool {
    true
}

fn default_language() -> String {
    "en".to_string()
}

fn default_min_chars() -> u32 {
    2
}

fn default_max_chars() -> u32 {
    200
}

fn default_block_commands() -> bool {
    true
}

fn default_true() -> bool {
    true
}

fn default_lang_min_chars() -> u32 {
    2
}

fn default_speak_template() -> String {
    "{nick}. {text}".to_string()
}

fn default_inherit_volume() -> f32 {
    -1.0
}

impl Default for TwitchTtsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            channel: String::new(),
            nick: String::new(),
            oauth_token: String::new(),
            oauth_client_id: String::new(),
            speak_chat: default_speak_chat(),
            include_username: default_include_username(),
            language: default_language(),
            min_chars: default_min_chars(),
            max_chars: default_max_chars(),
            block_commands: default_block_commands(),
            ignore_users: Vec::new(),
            strip_emotes: true,
            strip_emoji: true,
            emote_sources: TwitchEmoteSources::default(),
            detect_language: true,
            lang_min_chars: default_lang_min_chars(),
            enabled_languages: Vec::new(),
            nick_replacements: Vec::new(),
            include_builtin_profanity: true,
            pause_style: TwitchPauseStyle::default(),
            speak_template: default_speak_template(),
            audio_output_device_id: String::new(),
            audio_output_device_label: String::new(),
            speech_rate: 0.0,
            speech_volume: -1.0,
            max_queue_items: 0,
        }
    }
}

impl TwitchTtsSettings {
    pub fn effective_speech_rate(&self, root: f32) -> f32 {
        if self.speech_rate > 0.0 {
            self.speech_rate
        } else {
            root
        }
    }

    pub fn effective_speech_volume(&self, root: f32) -> f32 {
        if self.speech_volume >= 0.0 {
            self.speech_volume
        } else {
            root
        }
    }

    pub fn effective_max_queue_items(&self) -> u32 {
        if self.max_queue_items > 0 {
            self.max_queue_items
        } else {
            6
        }
    }

    pub fn normalized_channel(&self) -> String {
        let trimmed = self.channel.trim().trim_start_matches('#').to_lowercase();
        if trimmed.is_empty() {
            return String::new();
        }
        format!("#{trimmed}")
    }

    pub fn channel_login(&self) -> String {
        self.channel.trim().trim_start_matches('#').to_lowercase()
    }

    pub fn resolve_client_id(&self) -> String {
        let from_settings = self.oauth_client_id.trim();
        if !from_settings.is_empty() {
            return from_settings.to_string();
        }
        crate::emotes::DEFAULT_TWITCH_CLIENT_ID.to_string()
    }

    pub fn validate_for_connect(&self) -> Result<(), String> {
        if self.normalized_channel().is_empty() {
            return Err("channel is required".into());
        }
        if self.nick.trim().is_empty() {
            return Err("nick is required".into());
        }
        if self.oauth_token.trim().is_empty() {
            return Err(
                "oauth token is required — use Twitch CLI (chat:read) or Device Code Flow; see TTS Twitch tab help"
                    .into(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwitchChatMessage {
    pub id: String,
    pub user: String,
    pub display_name: String,
    pub text: String,
    pub speak_text: String,
    /// Message body after emote/emoji cleanup (before nick prefix).
    #[serde(default)]
    pub clean_text: String,
    /// Nick string sent to TTS (after replacements).
    #[serde(default)]
    pub spoken_nick: String,
    pub channel: String,
    pub language: String,
    pub is_mod: bool,
    pub is_subscriber: bool,
    /// `true` when message passed TTS filters in the IRC layer.
    #[serde(default)]
    pub speakable: bool,
}
