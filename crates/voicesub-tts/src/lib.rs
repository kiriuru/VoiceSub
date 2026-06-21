//! TTS optional module — configuration, speech queues, and Rust-side playback orchestration.

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
mod upstream_retry;

pub use config::TtsConfig;
pub use google_fetch::{GOOGLE_TTS_MAX_CHARS, fetch_google_tts_browser};
pub use ipc::{
    TTS_WINDOW_LABEL, bind_window_process, build_tts_module_url, speech_queue_item_id,
    tts_webview_data_dir, validate_twitch_oauth_url,
};
pub use oauth_bridge::TwitchOAuthBridge;
pub use python_runtime::{
    PythonRuntimeKind, PythonRuntimeStatus, embedded_binary_path, normalize_tts_lang,
    probe_python_runtime, run_google_tts_fetch,
};
pub use queue::{ChannelEnqueueResult, SpeechQueueItem};
pub use service::TtsModuleService;
pub use speech_pipeline::{SpeechPipelineError, TtsSpeechPipeline};
pub use subtitle_speech::{SubtitleSpeechPlanner, TtsSpeechSettings};
