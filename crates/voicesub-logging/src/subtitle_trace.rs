use std::path::Path;
use std::sync::{Mutex, OnceLock};

use serde_json::{Value, json};

use crate::diagnostics::is_subtitle_trace_enabled;
use crate::jsonl_trace::JsonlTraceLog;

static SUBTITLE_TRACE: OnceLock<Mutex<Option<JsonlTraceLog>>> = OnceLock::new();

fn store() -> &'static Mutex<Option<JsonlTraceLog>> {
    SUBTITLE_TRACE.get_or_init(|| Mutex::new(None))
}

pub fn configure_subtitle_trace_log(logs_dir: &Path) {
    let mut guard = store().lock().unwrap_or_else(|e| e.into_inner());
    if is_subtitle_trace_enabled() {
        *guard = Some(JsonlTraceLog::open(logs_dir, "subtitle-trace.jsonl"));
    } else {
        *guard = None;
    }
}

pub fn subtitle_trace(lane: &str, component: &str, event: &str, fields: Value) {
    if !is_subtitle_trace_enabled() {
        return;
    }
    let guard = store().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(log) = guard.as_ref() {
        log.append(lane, component, event, fields.clone());
    }
    tracing::debug!(
        target: "voicesub.subtitle",
        lane = lane,
        component = component,
        event = event,
        fields = %fields,
        "subtitle_trace"
    );
}

pub fn subtitle_trace_mapping(lane: &str, component: &str, event: &str, payload: Option<&Value>) {
    match payload {
        Some(fields) => subtitle_trace(lane, component, event, fields.clone()),
        None => subtitle_trace(lane, component, event, json!({})),
    }
}
