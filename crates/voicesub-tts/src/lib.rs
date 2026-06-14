//! TTS optional module — configuration persistence and speech queue prototype.

mod config;
mod async_runtime;
pub mod ipc;
mod oauth_bridge;
mod python_runtime;
mod channel_queue;
mod queue;
mod service;
mod subtitle_speech;
mod trace;
mod google_fetch;
mod playback_policy;
mod channel_orchestrator;
mod speech_pipeline;

pub use config::{
    normalize_playback_mode, normalize_tts_config, normalize_tts_provider, TtsConfig,
    TtsConfigStore, PLAYBACK_MODE_BROWSER, PLAYBACK_MODE_NATIVE, PLAYBACK_MODE_SONIC, TTS_PROVIDER_BROWSER_GOOGLE,
    TTS_PROVIDER_PYTHON_STDLIB,
};
pub use python_runtime::{
    embedded_binary_path, normalize_tts_lang, probe_python_runtime, run_google_tts_fetch,
    PythonRuntimeKind, PythonRuntimeStatus,
};
pub use oauth_bridge::TwitchOAuthBridge;
pub use channel_queue::{
    DualChannelSpeechQueue, ChannelQueueError, CHANNEL_SPEECH, CHANNEL_TWITCH,
};
pub use queue::{ChannelEnqueueResult, SpeechQueue, SpeechQueueItem, SpeechQueueState};
pub use ipc::{
    bind_window_process, build_tts_module_url, speech_queue_item_id, tts_webview_data_dir,
    validate_twitch_oauth_url, TTS_WINDOW_LABEL,
};
pub use service::TtsModuleService;
pub use subtitle_speech::{SubtitleSpeechPlanner, TtsSpeechSettings};
pub use google_fetch::{
    chunk_text_for_google_tts, fetch_google_tts_browser, prefetch_tts_line, GOOGLE_TTS_MAX_CHARS,
};
pub use speech_pipeline::TtsSpeechPipeline;
pub use channel_orchestrator::CompletionWaiter;
pub use voicesub_twitch::{
    SourceTextReplacementPair, SourceTextReplacementSettings,
    TwitchChatService, TwitchConnectionState, TwitchConnectionStatus, TwitchTtsSettings,
};
