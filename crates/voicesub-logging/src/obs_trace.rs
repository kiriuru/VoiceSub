use std::path::Path;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

use crate::diagnostics::is_obs_trace_enabled;
use crate::jsonl_trace::JsonlTraceLog;

static OBS_TRACE: OnceLock<Mutex<Option<JsonlTraceLog>>> = OnceLock::new();

fn store() -> &'static Mutex<Option<JsonlTraceLog>> {
    OBS_TRACE.get_or_init(|| Mutex::new(None))
}

pub fn configure_obs_trace_log(logs_dir: &Path) {
    let mut guard = store().lock().unwrap_or_else(|e| e.into_inner());
    if is_obs_trace_enabled() {
        *guard = Some(JsonlTraceLog::open(logs_dir, "obs-trace.jsonl"));
    } else {
        *guard = None;
    }
}

pub fn obs_trace(lane: &str, component: &str, event: &str, fields: Value) {
    if !is_obs_trace_enabled() {
        return;
    }
    let guard = store().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(log) = guard.as_ref() {
        log.append(lane, component, event, fields.clone());
    }
    tracing::debug!(
        target: "voicesub.obs",
        lane = lane,
        component = component,
        event = event,
        fields = %fields,
        "obs_trace"
    );
}
