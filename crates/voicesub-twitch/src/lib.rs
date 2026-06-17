//! Twitch IRC chat for the TTS module (read chat, filter, broadcast to UI).

mod emoji;
mod emotes;
mod error;
mod filter;
mod irc;
mod lang;
mod links;
mod pipeline;
mod replacements;
mod service;
mod settings;
mod source_text_replacement;
mod symbols;
mod tls;
mod trace;

pub use lang::{
    TWITCH_TOP_LANGUAGE_CODES, language_allowed, resolve_message_language, strip_twitch_mentions,
};

pub use emotes::{EmoteRegistry, EmoteSets};
pub use error::TwitchError;
pub use filter::should_speak_message;
pub use service::{
    EventBroadcaster, TwitchChatService, TwitchConnectionState, TwitchConnectionStatus,
};
pub use settings::{
    TwitchChatMessage, TwitchEmoteSources, TwitchPauseStyle, TwitchReplacement, TwitchTtsSettings,
    normalize_speak_template, normalize_twitch_settings, pause_separator,
};
pub use source_text_replacement::{
    SourceTextReplacementPair, SourceTextReplacementSettings, apply_source_text_replacement,
    settings_from_config_value, settings_from_section_value,
};
pub use tls::init_crypto_provider;
