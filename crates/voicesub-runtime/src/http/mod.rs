//! Axum HTTP server — static routes + WebSocket endpoints.

mod asr_diagnostics;
mod background_tasks;
mod devices;
mod exports;
mod logs;
mod loopback_auth;
mod metrics;
mod openai;
mod partial_emit;
mod profiles;
mod router;
mod runtime;
mod runtime_state;
mod settings;
mod state;
mod tts_proxy;
mod tts_python;
mod twitch_oauth;
mod ui_sync;
mod update_service;
mod updates;

pub use background_tasks::BackgroundTaskRegistry;
pub use loopback_auth::{LOOPBACK_TOKEN_HEADER, LoopbackAuth};
pub use metrics::RuntimeMetricsCollector;
pub use partial_emit::{PartialEmitCoordinator, partial_emit_settings_from_config};
pub use router::build_router;
pub use runtime_state::{RuntimeStatusBroadcaster, spawn_runtime_heartbeat};
pub use state::{HttpState, StylePresetsFn};
pub use update_service::{check_now, spawn_startup_check};
