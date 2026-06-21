use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
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

impl TwitchError {
    /// Whether a failed IRC session should be retried after backoff.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::InvalidSettings(_) | Self::AlreadyConnected | Self::NotConnected => false,
            Self::Tls(_) => true,
            Self::Irc(message) => !is_auth_failure_message(message),
        }
    }
}

fn is_auth_failure_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("login authentication failed")
        || lower.contains("improperly formatted auth")
        || lower.contains("invalid nick")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_failures_are_not_retryable() {
        let err = TwitchError::Irc(
            "Twitch login authentication failed — check nick and OAuth token".into(),
        );
        assert!(!err.is_retryable());
    }

    #[test]
    fn network_errors_are_retryable() {
        assert!(TwitchError::Irc("tcp connect failed: connection reset".into()).is_retryable());
        assert!(TwitchError::Tls("handshake failed".into()).is_retryable());
        assert!(TwitchError::Irc(
            "peer closed connection without sending TLS close_notify".into()
        )
        .is_retryable());
    }

    #[test]
    fn invalid_settings_are_not_retryable() {
        assert!(!TwitchError::InvalidSettings("missing nick".into()).is_retryable());
    }
}
