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
    language_allowed, resolve_message_language, strip_twitch_mentions, TWITCH_TOP_LANGUAGE_CODES,
};

pub use emotes::{EmoteRegistry, EmoteSets};
pub use error::TwitchError;
pub use filter::should_speak_message;
pub use service::{EventBroadcaster, TwitchChatService, TwitchConnectionState, TwitchConnectionStatus};
pub use settings::{
    normalize_speak_template, pause_separator, TwitchChatMessage, TwitchEmoteSources, TwitchPauseStyle,
    TwitchReplacement, TwitchTtsSettings,
};
pub use source_text_replacement::{
    apply_source_text_replacement, settings_from_config_value, settings_from_section_value,
    SourceTextReplacementPair, SourceTextReplacementSettings,
};
pub use tls::init_crypto_provider;
