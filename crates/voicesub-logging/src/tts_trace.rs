use std::path::Path;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

use crate::diagnostics::is_tts_trace_enabled;
use crate::jsonl_trace::JsonlTraceLog;

static TTS_TRACE: OnceLock<Mutex<Option<JsonlTraceLog>>> = OnceLock::new();

fn store() -> &'static Mutex<Option<JsonlTraceLog>> {
    TTS_TRACE.get_or_init(|| Mutex::new(None))
}

pub fn configure_tts_trace_log(logs_dir: &Path) {
    let mut guard = store().lock().unwrap_or_else(|e| e.into_inner());
    if is_tts_trace_enabled() {
        *guard = Some(JsonlTraceLog::open(logs_dir, "tts-trace.jsonl"));
    } else {
        *guard = None;
    }
}

pub fn tts_trace(component: &str, event: &str, fields: Value) {
    if !is_tts_trace_enabled() {
        return;
    }
    let guard = store().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(log) = guard.as_ref() {
        log.append("tts", component, event, fields.clone());
    }
    tracing::debug!(
        target: "voicesub.tts",
        component = component,
        event = event,
        fields = %fields,
        "tts_trace"
    );
}
