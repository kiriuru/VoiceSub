use serde::Serialize;
use serde_json::{json, Value};

use crate::error_codes::native_status;
use crate::settings::ObsCaptionSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    #[default]
    Disabled,
    Disconnected,
    Connecting,
    Connected,
    AuthFailed,
    Error,
}

impl ConnectionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::AuthFailed => "auth_failed",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ObsCaptionDiagnostics {
    pub connection_state: ConnectionState,
    pub connected: bool,
    pub active: bool,
    pub stream_output_active: Option<bool>,
    pub stream_output_reconnecting: Option<bool>,
    pub native_caption_ready: bool,
    pub native_caption_status: Option<String>,
    pub reconnect_attempt_count: u32,
    pub last_send_used_active_connection: bool,
    pub last_send_waited_for_connection: bool,
    pub last_error: Option<String>,
    pub last_caption_text: Option<String>,
    pub last_caption_sent_at_utc: Option<String>,
    pub last_debug_text: Option<String>,
    pub last_debug_input_name: Option<String>,
    pub obs_websocket_version: Option<String>,
    pub obs_studio_version: Option<String>,
}

impl ObsCaptionDiagnostics {
    pub fn to_value(&self, settings: &ObsCaptionSettings) -> Value {
        let mut connection_state = self.connection_state;
        if connection_state == ConnectionState::Disabled && settings.should_connect() {
            connection_state = ConnectionState::Disconnected;
        }
        let native_enabled = settings.native_enabled();
        let mut native_caption_status = self.native_caption_status.clone();
        if native_enabled && native_caption_status.is_none() {
            native_caption_status = if !self.connected {
                Some(native_status::NOT_CONNECTED.into())
            } else if self.stream_output_active == Some(true) {
                Some(native_status::STREAM_ACTIVE.into())
            } else if self.stream_output_active == Some(false) {
                Some(native_status::STREAM_INACTIVE.into())
            } else {
                Some(native_status::READINESS_PENDING.into())
            };
        }
        json!({
            "enabled": settings.enabled,
            "output_mode": settings.output_mode,
            "host": settings.host,
            "port": settings.port,
            "password_configured": !settings.password.is_empty(),
            "connection_state": connection_state.as_str(),
            "send_partials": settings.send_partials,
            "partial_throttle_ms": settings.partial_throttle_ms,
            "min_partial_delta_chars": settings.min_partial_delta_chars,
            "final_replace_delay_ms": settings.final_replace_delay_ms,
            "clear_after_ms": settings.clear_after_ms,
            "avoid_duplicate_text": settings.avoid_duplicate_text,
            "connected": self.connected,
            "active": self.active,
            "stream_output_active": self.stream_output_active,
            "stream_output_reconnecting": self.stream_output_reconnecting,
            "native_caption_ready": self.native_caption_ready,
            "native_caption_status": native_caption_status,
            "transport": "obs-websocket",
            "request_type": "SendStreamCaption",
            "debug_request_type": if settings.debug_text_input_enabled() { Some("SetInputSettings") } else { None },
            "debug_text_input_enabled": settings.debug_text_input_enabled(),
            "debug_text_input_name": if settings.debug_text_input_enabled() { Some(settings.debug_input_name.clone()) } else { None },
            "debug_text_input_send_partials": settings.debug_send_partials,
            "reconnect_attempt_count": self.reconnect_attempt_count,
            "last_send_used_active_connection": self.last_send_used_active_connection,
            "last_send_waited_for_connection": self.last_send_waited_for_connection,
            "last_error": self.last_error,
            "last_caption_text": self.last_caption_text,
            "last_caption_sent_at_utc": self.last_caption_sent_at_utc,
            "last_debug_text": self.last_debug_text,
            "last_debug_input_name": self.last_debug_input_name,
            "obs_websocket_version": self.obs_websocket_version,
            "obs_studio_version": self.obs_studio_version,
        })
    }
}
