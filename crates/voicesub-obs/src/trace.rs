use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};
use voicesub_logging::{StructuredRuntimeLogger, obs_trace};

use crate::diagnostics::ConnectionState;

pub type StructuredLogFn = Arc<dyn Fn(&str, &str, Value) + Send + Sync>;

const CHANNEL: &str = "obs_caption_output";
const SOURCE: &str = "obs_caption_output";

pub fn structured_log_from_runtime_logger(logger: Arc<StructuredRuntimeLogger>) -> StructuredLogFn {
    Arc::new(move |_channel, event, fields| {
        let mut map = BTreeMap::new();
        if let Some(obj) = fields.as_object() {
            for (key, value) in obj {
                map.insert(key.clone(), value.clone());
            }
        }
        logger.log(CHANNEL, event, Some(SOURCE), Some(map));
    })
}

#[derive(Clone, Default)]
pub struct ObsCaptionLog {
    structured: Option<StructuredLogFn>,
}

impl ObsCaptionLog {
    pub fn new(structured: Option<StructuredLogFn>) -> Self {
        Self { structured }
    }

    fn emit(&self, event: &str, fields: Value) {
        if let Some(ref logger) = self.structured {
            logger(CHANNEL, event, fields.clone());
        }
        obs_trace(CHANNEL, SOURCE, event, fields);
    }

    pub(crate) fn service_started(&self) {
        self.emit("obs_service_started", json!({}));
    }

    pub(crate) fn service_stopped(&self) {
        self.emit("obs_service_stopped", json!({}));
    }

    pub(crate) fn live_settings_applied(
        &self,
        enabled: bool,
        should_connect: bool,
        output_mode: &str,
        connection_key_changed: bool,
    ) {
        self.emit(
            "obs_live_settings_applied",
            json!({
                "enabled": enabled,
                "should_connect": should_connect,
                "output_mode": output_mode,
                "connection_key_changed": connection_key_changed,
            }),
        );
    }

    pub(crate) fn connection_state_changed(&self, state: ConnectionState, error: Option<&str>) {
        let mut fields = json!({ "state": state.as_str() });
        if let Some(err) = error.filter(|s| !s.is_empty())
            && let Some(obj) = fields.as_object_mut()
        {
            obj.insert("error".into(), json!(err));
        }
        self.emit("obs_connection_state_changed", fields);
    }

    pub(crate) fn connection_lost(&self, error: &str) {
        self.emit("obs_connection_lost", json!({ "error": error }));
    }

    pub(crate) fn partial_throttled(&self, text_len: usize, elapsed_ms: Option<u64>) {
        self.emit(
            "obs_partial_throttled",
            json!({
                "text_len": text_len,
                "elapsed_ms": elapsed_ms,
            }),
        );
    }

    pub(crate) fn send_skipped(&self, reason: &str, fields: Value) {
        let mut body = json!({ "reason": reason });
        if let Some(obj) = body.as_object_mut()
            && let Some(extra) = fields.as_object()
        {
            for (key, value) in extra {
                obj.insert(key.clone(), value.clone());
            }
        }
        self.emit("obs_caption_send_skipped", body);
    }

    pub(crate) fn caption_sent(
        &self,
        text_len: usize,
        send_stream: bool,
        mirror_debug: bool,
        used_active_connection: bool,
        waited_for_connection: bool,
    ) {
        self.emit(
            "obs_caption_sent",
            json!({
                "text_len": text_len,
                "send_stream": send_stream,
                "mirror_debug": mirror_debug,
                "used_active_connection": used_active_connection,
                "waited_for_connection": waited_for_connection,
            }),
        );
    }

    pub(crate) fn debug_mirror_sent(&self, text_len: usize) {
        self.emit("obs_debug_mirror_sent", json!({ "text_len": text_len }));
    }

    pub(crate) fn stream_output_inactive(&self) {
        self.emit("obs_stream_output_inactive", json!({}));
    }

    pub(crate) fn caption_send_failed(&self, error: &str) {
        self.emit("obs_caption_send_failed", json!({ "error": error }));
    }

    pub(crate) fn payload_routed(&self, sequence: u64, output_mode: &str, text_len: usize) {
        self.emit(
            "obs_payload_routed",
            json!({
                "sequence": sequence,
                "output_mode": output_mode,
                "text_len": text_len,
            }),
        );
    }
}
