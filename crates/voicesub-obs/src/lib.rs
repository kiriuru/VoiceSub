//! OBS WebSocket closed captions output.

mod auth;
mod client;
mod diagnostics;
mod error_codes;
mod service;
mod settings;
mod text;
mod trace;

#[cfg(test)]
mod send_integration_tests;

#[cfg(test)]
pub use client::MockObsClient;
pub use diagnostics::ObsCaptionDiagnostics;
pub use service::ObsCaptionService;
pub use settings::ObsCaptionSettings;
pub use text::{normalize_text, select_payload_text, should_throttle_partial_update};
pub use trace::{ObsCaptionLog, StructuredLogFn, structured_log_from_runtime_logger};
