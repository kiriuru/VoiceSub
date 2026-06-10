use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("invalid process id")]
    InvalidProcessId,

    #[error("device not found: {0}")]
    DeviceNotFound(String),

    #[cfg(windows)]
    #[error("windows audio API error: {0}")]
    Windows(#[from] windows::core::Error),

    #[error("audio routing failed: {0}")]
    RoutingFailed(String),

    #[error("playback failed: {0}")]
    PlaybackFailed(String),
}
