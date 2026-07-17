//! Runtime orchestration — wires HTTP, WS, browser worker, subtitle and translation pipeline.

pub mod http;

mod browser_event_builder;
mod browser_speech_source;
mod local_asr_speech_source;
mod segment_state;
mod service;
mod trace;
mod transcript_controller;

pub use http::{
    BackgroundTaskRegistry, HttpState, LOOPBACK_TOKEN_HEADER, LoopbackAuth, PartialEmitCoordinator,
    RuntimeMetricsCollector, StylePresetsFn, build_router, partial_emit_settings_from_config,
};
pub use service::{RuntimeError, RuntimeHandle, RuntimeService, SubtitlePayloadListener};
pub use voicesub_ws::RuntimeStateSnapshot;
