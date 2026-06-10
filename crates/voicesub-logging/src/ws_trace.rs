use std::path::Path;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

use crate::diagnostics::is_ws_trace_enabled;
use crate::jsonl_trace::JsonlTraceLog;

static WS_TRACE: OnceLock<Mutex<Option<JsonlTraceLog>>> = OnceLock::new();

fn store() -> &'static Mutex<Option<JsonlTraceLog>> {
    WS_TRACE.get_or_init(|| Mutex::new(None))
}

pub fn configure_ws_trace_log(logs_dir: &Path) {
    let mut guard = store().lock().unwrap_or_else(|e| e.into_inner());
    if is_ws_trace_enabled() {
        *guard = Some(JsonlTraceLog::open(logs_dir, "ws-trace.jsonl"));
    } else {
        *guard = None;
    }
}

pub fn ws_trace(lane: &str, component: &str, event: &str, fields: Value) {
    if !is_ws_trace_enabled() {
        return;
    }
    let guard = store().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(log) = guard.as_ref() {
        log.append(lane, component, event, fields.clone());
    }
    tracing::debug!(
        target: "voicesub.ws",
        lane = lane,
        component = component,
        event = event,
        fields = %fields,
        "ws_trace"
    );
}
