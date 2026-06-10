use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Known inbound worker message types (`/ws/asr_worker`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WsMessageType {
    ExternalAsrUpdate,
    BrowserAsrStatus,
    BrowserAsrHeartbeat,
    Hello,
    Unknown,
}

pub fn parse_worker_message_type(raw: &str) -> WsMessageType {
    match raw.trim().to_ascii_lowercase().as_str() {
        "external_asr_update" => WsMessageType::ExternalAsrUpdate,
        "browser_asr_status" => WsMessageType::BrowserAsrStatus,
        "browser_asr_heartbeat" => WsMessageType::BrowserAsrHeartbeat,
        "hello" => WsMessageType::Hello,
        _ => WsMessageType::Unknown,
    }
}

/// Generic JSON envelope used across WS endpoints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WsMessage {
    #[serde(rename = "type")]
    pub message_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport_id: Option<u64>,
}

impl WsMessage {
    pub fn hello_events() -> Self {
        Self {
            message_type: "hello".into(),
            message: Some("connected".into()),
            payload: None,
            transport_id: None,
        }
    }

    pub fn parsed_type(&self) -> WsMessageType {
        parse_worker_message_type(&self.message_type)
    }
}

/// Outbound handshake for `/ws/asr_worker` (SST `BrowserAsrService.send_hello`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AsrWorkerHello {
    #[serde(rename = "type")]
    pub message_type: String,
    pub message: String,
    pub transport_id: u64,
}

impl AsrWorkerHello {
    pub fn new(transport_id: u64) -> Self {
        Self {
            message_type: "hello".into(),
            message: "browser_asr_worker_connected".into(),
            transport_id,
        }
    }
}

pub type EventsHello = WsMessage;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_worker_message_types() {
        assert_eq!(
            parse_worker_message_type("external_asr_update"),
            WsMessageType::ExternalAsrUpdate
        );
        assert_eq!(
            parse_worker_message_type("BROWSER_ASR_HEARTBEAT"),
            WsMessageType::BrowserAsrHeartbeat
        );
    }

    #[test]
    fn asr_worker_hello_matches_sst_contract() {
        let hello = AsrWorkerHello::new(3);
        let json = serde_json::to_value(&hello).expect("serialize");
        assert_eq!(json["type"], "hello");
        assert_eq!(json["message"], "browser_asr_worker_connected");
        assert_eq!(json["transport_id"], 3);
    }

    #[test]
    fn events_hello_matches_sst_contract() {
        let hello = WsMessage::hello_events();
        assert_eq!(hello.message_type, "hello");
        assert_eq!(hello.message.as_deref(), Some("connected"));
    }
}
