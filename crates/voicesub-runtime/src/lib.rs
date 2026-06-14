//! Runtime orchestration — wires HTTP, WS, browser worker, subtitle and translation pipeline.

pub mod http;

mod browser_event_builder;
mod browser_speech_source;
mod segment_state;
mod service;
mod trace;
mod transcript_controller;

pub use http::{
    build_router, partial_emit_settings_from_config, HttpState, PartialEmitCoordinator,
    RuntimeMetricsCollector, StylePresetsFn,
};
pub use service::{RuntimeError, RuntimeHandle, RuntimeService, SubtitlePayloadListener};
pub use voicesub_ws::RuntimeStateSnapshot;
