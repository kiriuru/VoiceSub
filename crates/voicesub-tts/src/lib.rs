//! TTS optional module — configuration persistence and speech queue prototype.

mod async_runtime;
mod channel_orchestrator;
mod channel_queue;
mod config;
mod google_fetch;
pub mod ipc;
mod oauth_bridge;
mod playback_policy;
mod python_runtime;
mod queue;
mod service;
mod speech_pipeline;
mod subtitle_speech;
mod trace;

pub use channel_orchestrator::CompletionWaiter;
pub use channel_queue::{
    CHANNEL_SPEECH, CHANNEL_TWITCH, ChannelQueueError, DualChannelSpeechQueue,
};
pub use config::{
    PLAYBACK_MODE_BROWSER, PLAYBACK_MODE_NATIVE, PLAYBACK_MODE_SONIC, TTS_PROVIDER_BROWSER_GOOGLE,
    TTS_PROVIDER_PYTHON_STDLIB, TtsConfig, TtsConfigStore, normalize_playback_mode,
    normalize_tts_config, normalize_tts_provider,
};
pub use google_fetch::{
    GOOGLE_TTS_MAX_CHARS, chunk_text_for_google_tts, fetch_google_tts_browser, prefetch_tts_line,
};
pub use ipc::{
    TTS_WINDOW_LABEL, bind_window_process, build_tts_module_url, speech_queue_item_id,
    tts_webview_data_dir, validate_twitch_oauth_url,
};
pub use oauth_bridge::TwitchOAuthBridge;
pub use python_runtime::{
    PythonRuntimeKind, PythonRuntimeStatus, embedded_binary_path, normalize_tts_lang,
    probe_python_runtime, run_google_tts_fetch,
};
pub use queue::{ChannelEnqueueResult, SpeechQueue, SpeechQueueItem, SpeechQueueState};
pub use service::TtsModuleService;
pub use speech_pipeline::TtsSpeechPipeline;
pub use subtitle_speech::{SubtitleSpeechPlanner, TtsSpeechSettings};
pub use voicesub_twitch::{
    SourceTextReplacementPair, SourceTextReplacementSettings, TwitchChatService,
    TwitchConnectionState, TwitchConnectionStatus, TwitchTtsSettings,
};
