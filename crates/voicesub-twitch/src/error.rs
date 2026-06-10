use thiserror::Error;

#[derive(Debug, Error)]
pub enum TwitchError {
    #[error("twitch irc: {0}")]
    Irc(String),

    #[error("already connected")]
    AlreadyConnected,

    #[error("not connected")]
    NotConnected,

    #[error("invalid settings: {0}")]
    InvalidSettings(String),

    #[error("tls error: {0}")]
    Tls(String),
}
