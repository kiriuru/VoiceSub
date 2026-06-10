//! Shared DTOs, enums, and WebSocket/API payload types (Layer 0).

pub mod asr;
pub mod version;
pub mod ws;

pub use asr::ExternalAsrUpdate;
pub use version::PROJECT_VERSION;
pub use ws::{parse_worker_message_type, AsrWorkerHello, EventsHello, WsMessage, WsMessageType};
