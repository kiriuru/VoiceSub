use crate::client::ObsClientError;

pub mod error {
    pub const PASSWORD_REQUIRED: &str = "password_required";
    pub const AUTH_FAILED: &str = "auth_failed";
    pub const CONNECTION_REFUSED: &str = "connection_refused";
    pub const CONNECTION_TIMEOUT: &str = "connection_timeout";
    pub const CONNECTION_FAILED: &str = "connection_failed";
    pub const CONNECTION_LOST: &str = "connection_lost";
    pub const PROTOCOL_ERROR: &str = "protocol_error";
    pub const REQUEST_FAILED: &str = "request_failed";
    pub const NOT_CONNECTED: &str = "not_connected";
    pub const SEND_FAILED: &str = "send_failed";
    pub const STREAM_NOT_RUNNING: &str = "stream_not_running";
}

pub mod native_status {
    pub const NOT_CONNECTED: &str = "not_connected";
    pub const STREAM_ACTIVE: &str = "stream_active";
    pub const STREAM_ACTIVE_RECONNECTING: &str = "stream_active_reconnecting";
    pub const STREAM_INACTIVE: &str = "stream_inactive";
    pub const STREAM_NOT_RUNNING: &str = "stream_not_running";
    pub const STREAM_DELIVERED: &str = "stream_delivered";
    pub const READINESS_PENDING: &str = "readiness_pending";
}

pub fn obs_client_error_code(err: &ObsClientError) -> &'static str {
    match err {
        ObsClientError::PasswordRequired => error::PASSWORD_REQUIRED,
        ObsClientError::AuthFailed => error::AUTH_FAILED,
        ObsClientError::RequestFailed { .. } => error::REQUEST_FAILED,
        ObsClientError::Protocol(_) => error::PROTOCOL_ERROR,
        ObsClientError::Io(io_err) => classify_io_error(io_err),
    }
}

fn classify_io_error(err: &tokio_tungstenite::tungstenite::Error) -> &'static str {
    let haystack = err.to_string().to_ascii_lowercase();
    if haystack.contains("10061")
        || haystack.contains("connection refused")
        || haystack.contains("actively refused")
        || haystack.contains("отверг запрос на подключение")
    {
        return error::CONNECTION_REFUSED;
    }
    if haystack.contains("10060") || haystack.contains("timed out") || haystack.contains("timeout")
    {
        return error::CONNECTION_TIMEOUT;
    }
    error::CONNECTION_FAILED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_password_required() {
        assert_eq!(
            obs_client_error_code(&ObsClientError::PasswordRequired),
            error::PASSWORD_REQUIRED
        );
    }

    #[test]
    fn maps_auth_failed() {
        assert_eq!(
            obs_client_error_code(&ObsClientError::AuthFailed),
            error::AUTH_FAILED
        );
    }
}
