use std::path::Path;
use std::sync::{Mutex, OnceLock};

use serde_json::{Value, json};

use crate::diagnostics::is_ui_trace_enabled;
use crate::jsonl_trace::JsonlTraceLog;

static UI_TRACE: OnceLock<Mutex<Option<JsonlTraceLog>>> = OnceLock::new();

fn store() -> &'static Mutex<Option<JsonlTraceLog>> {
    UI_TRACE.get_or_init(|| Mutex::new(None))
}

pub fn configure_ui_trace_log(logs_dir: &Path) {
    let mut guard = store().lock().unwrap_or_else(|e| e.into_inner());
    if is_ui_trace_enabled() {
        *guard = Some(JsonlTraceLog::open(logs_dir, "ui-trace.jsonl"));
    } else {
        *guard = None;
    }
}

pub fn ui_trace(surface: &str, phase: &str, event: &str, fields: Value) {
    if !is_ui_trace_enabled() {
        return;
    }
    let guard = store().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(log) = guard.as_ref() {
        log.append(surface, phase, event, fields.clone());
    }
    tracing::debug!(
        target: "voicesub.ui_trace",
        surface = surface,
        phase = phase,
        event = event,
        fields = %fields,
        "ui_trace"
    );
}

pub fn ui_trace_mapping(surface: &str, phase: &str, event: &str, payload: Option<&Value>) {
    let fields = match payload {
        Some(value) => value.clone(),
        None => json!({}),
    };
    ui_trace(surface, phase, event, fields);
}
