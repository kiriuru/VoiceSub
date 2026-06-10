use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Value};
use voicesub_logging::{ws_trace, StructuredRuntimeLogger};

pub type StructuredLogFn = Arc<dyn Fn(&str, &str, Value) + Send + Sync>;

pub const WS_LOG_CHANNEL: &str = "ws_events";
pub const WS_LOG_SOURCE: &str = "ws_manager";

pub fn structured_log_from_runtime_logger(
    logger: Arc<StructuredRuntimeLogger>,
) -> StructuredLogFn {
    Arc::new(move |_channel, event, fields| {
        let mut map = BTreeMap::new();
        if let Some(obj) = fields.as_object() {
            for (key, value) in obj {
                map.insert(key.clone(), value.clone());
            }
        }
        logger.log(WS_LOG_CHANNEL, event, Some(WS_LOG_SOURCE), Some(map));
    })
}

#[derive(Clone, Default)]
pub struct WsLog {
    structured: Option<StructuredLogFn>,
}

impl WsLog {
    pub fn new(structured: Option<StructuredLogFn>) -> Self {
        Self { structured }
    }

    fn emit(&self, event: &str, fields: Value) {
        if let Some(ref logger) = self.structured {
            logger(WS_LOG_CHANNEL, event, fields.clone());
        }
        ws_trace("ws", WS_LOG_SOURCE, event, fields);
    }

    pub(crate) fn connection_open(
        &self,
        socket_id: u64,
        connections_active: usize,
        outbound_queue_max: usize,
    ) {
        self.emit(
            "ws_connection_open",
            json!({
                "socket_id": socket_id,
                "connections_active": connections_active,
                "outbound_queue_max": outbound_queue_max,
            }),
        );
    }

    pub(crate) fn connection_closed(&self, socket_id: u64, connections_active: usize) {
        self.emit(
            "ws_connection_closed",
            json!({
                "socket_id": socket_id,
                "connections_active": connections_active,
            }),
        );
    }

    pub(crate) fn broadcast(&self, message_type: &str, connection_count: usize) {
        if message_type == "runtime_update" {
            return;
        }
        self.emit(
            "ws_broadcast",
            json!({
                "message_type": message_type,
                "connection_count": connection_count,
            }),
        );
    }

    pub(crate) fn outbound_queue_pressure(
        &self,
        queue_depth: usize,
        queue_max: usize,
        message_type: &str,
    ) {
        self.emit(
            "ws_outbound_queue_pressure",
            json!({
                "queue_depth": queue_depth,
                "queue_max": queue_max,
                "message_type": message_type,
            }),
        );
    }

    pub(crate) fn outbound_queue_drop_oldest(&self, message_type: &str) {
        self.emit(
            "ws_outbound_queue_drop_oldest",
            json!({ "message_type": message_type }),
        );
    }
}
