//! Shared DTOs, enums, and WebSocket/API payload types (Layer 0).

pub mod asr;
pub mod time;
pub mod version;
pub mod ws;

pub use asr::ExternalAsrUpdate;
pub use time::{epoch_secs_to_rfc3339, utc_now_rfc3339};
pub use version::{
    DEFAULT_GITHUB_REPO, LEGACY_GITHUB_REPO, PROJECT_VERSION, RELEASE_TRACK,
    build_version_info_payload, extract_latest_github_release, is_remote_version_newer,
    release_url_for,
};
pub use ws::{AsrWorkerHello, EventsHello, WsMessage, WsMessageType, parse_worker_message_type};
