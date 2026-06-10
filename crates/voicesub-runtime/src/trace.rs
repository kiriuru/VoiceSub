use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Value};
use voicesub_logging::{pipeline_trace, StructuredRuntimeLogger};

pub type StructuredLogFn = Arc<dyn Fn(&str, &str, Value) + Send + Sync>;

pub const RUNTIME_METRICS_CHANNEL: &str = "runtime_metrics";
pub const RUNTIME_ORCHESTRATOR_SOURCE: &str = "runtime_orchestrator";
pub const RUNTIME_STATE_SOURCE: &str = "runtime_state_controller";
pub const RUNTIME_INGEST_SOURCE: &str = "runtime_ingest";

pub fn structured_log_from_runtime_logger(
    logger: Arc<StructuredRuntimeLogger>,
) -> StructuredLogFn {
    Arc::new(move |source, event, fields| {
        let mut map = BTreeMap::new();
        if let Some(obj) = fields.as_object() {
            for (key, value) in obj {
                map.insert(key.clone(), value.clone());
            }
        }
        logger.log(RUNTIME_METRICS_CHANNEL, event, Some(source), Some(map));
    })
}

#[derive(Clone, Default)]
pub struct RuntimePipelineLog {
    structured: Option<StructuredLogFn>,
}

impl RuntimePipelineLog {
    pub fn new(structured: Option<StructuredLogFn>) -> Self {
        Self { structured }
    }

    fn emit(&self, lane: &str, source: &str, event: &str, fields: Value) {
        if let Some(ref logger) = self.structured {
            logger(source, event, fields.clone());
        }
        pipeline_trace(lane, source, event, fields);
    }

    pub fn start_begin(&self) {
        self.emit(
            "runtime_lifecycle",
            RUNTIME_ORCHESTRATOR_SOURCE,
            "start_begin",
            json!({}),
        );
    }

    pub fn start_complete(&self, phase: &str, worker_pid: Option<u32>) {
        self.emit(
            "runtime_api",
            RUNTIME_ORCHESTRATOR_SOURCE,
            "runtime_start_complete",
            json!({
                "phase": phase,
                "worker_pid": worker_pid,
            }),
        );
    }

    pub fn stop_begin(&self) {
        self.emit(
            "runtime_lifecycle",
            RUNTIME_ORCHESTRATOR_SOURCE,
            "stop_begin",
            json!({}),
        );
    }

    pub fn stop_complete(&self) {
        self.emit(
            "runtime_api",
            RUNTIME_ORCHESTRATOR_SOURCE,
            "runtime_stop_complete",
            json!({}),
        );
    }

    pub fn state_changed(
        &self,
        from_status: &str,
        to_status: &str,
        from_running: bool,
        to_running: bool,
        last_error: Option<&str>,
    ) {
        self.emit(
            "runtime_state",
            RUNTIME_ORCHESTRATOR_SOURCE,
            "state_changed",
            json!({
                "from_status": from_status,
                "to_status": to_status,
                "from_is_running": from_running,
                "to_is_running": to_running,
                "last_error": last_error,
            }),
        );
    }

    pub fn runtime_status_broadcast(&self, important_change: bool, heartbeat: bool) {
        self.emit(
            "runtime_state",
            RUNTIME_STATE_SOURCE,
            "runtime_status_broadcast",
            json!({
                "important_change": important_change,
                "heartbeat": heartbeat,
            }),
        );
    }

    pub fn runtime_status_duplicate_suppressed(&self) {
        self.emit(
            "runtime_state",
            RUNTIME_STATE_SOURCE,
            "runtime_status_duplicate_suppressed",
            json!({}),
        );
    }

    pub fn runtime_status_heartbeat_sent(&self) {
        self.emit(
            "runtime_state",
            RUNTIME_STATE_SOURCE,
            "runtime_status_heartbeat_sent",
            json!({}),
        );
    }

    pub fn asr_ingest_partial_suppressed(&self, segment_id: &str) {
        self.emit(
            "runtime_ingest",
            RUNTIME_INGEST_SOURCE,
            "asr_ingest_partial_suppressed",
            json!({ "segment_id": segment_id }),
        );
    }

    pub fn asr_ingest_published(&self, is_final: bool, sequence: u64, text_len: usize) {
        let event = if is_final {
            "asr_ingest_final_published"
        } else {
            "asr_ingest_partial_published"
        };
        self.emit(
            "runtime_ingest",
            RUNTIME_INGEST_SOURCE,
            event,
            json!({
                "sequence": sequence,
                "text_len": text_len,
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn structured_callback_receives_runtime_events() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_cb = seen.clone();
        let log = RuntimePipelineLog::new(Some(Arc::new(
            move |source, event, fields| {
                seen_cb
                    .lock()
                    .unwrap()
                    .push((source.to_string(), event.to_string(), fields));
            },
        )));
        log.runtime_status_duplicate_suppressed();
        let guard = seen.lock().unwrap();
        assert_eq!(guard.len(), 1);
        assert_eq!(guard[0].0, RUNTIME_STATE_SOURCE);
        assert_eq!(guard[0].1, "runtime_status_duplicate_suppressed");
    }
}
