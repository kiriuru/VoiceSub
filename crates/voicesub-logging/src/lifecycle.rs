//! Process lifecycle markers for graceful shutdown vs abnormal exit detection.

use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::diagnostics::is_deep_diagnostics_enabled;
use crate::pipeline_trace;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionExitState {
    Running,
    Graceful,
    Panic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionLifecycleRecord {
    pub pid: u32,
    pub version: String,
    pub started_utc_secs: u64,
    #[serde(default)]
    pub ended_utc_secs: Option<u64>,
    pub state: SessionExitState,
    #[serde(default)]
    pub reason: String,
    #[serde(default = "default_detail")]
    pub detail: Value,
}

fn default_detail() -> Value {
    Value::Null
}

pub fn session_lifecycle_path(project_root: &Path) -> PathBuf {
    project_root.join("logs").join("session-lifecycle.json")
}

fn utc_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn write_record(path: &Path, record: &SessionLifecycleRecord) {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let _ = fs::create_dir_all(parent);
    if let Ok(text) = serde_json::to_string_pretty(record) {
        let _ = fs::write(path, text);
    }
}

pub fn read_session_lifecycle_record(project_root: &Path) -> Option<SessionLifecycleRecord> {
    let text = fs::read_to_string(session_lifecycle_path(project_root)).ok()?;
    serde_json::from_str(&text).ok()
}

fn deep_lifecycle_event(component: &str, event: &str, fields: Value) {
    if !is_deep_diagnostics_enabled() {
        return;
    }
    pipeline_trace("desktop", component, event, fields.clone());
    info!(
        target: "voicesub.lifecycle",
        component = component,
        event = event,
        fields = %fields,
        "lifecycle"
    );
}

pub fn inspect_previous_session(project_root: &Path) {
    let Some(prev) = read_session_lifecycle_record(project_root) else {
        return;
    };
    if prev.state != SessionExitState::Running {
        return;
    }
    warn!(
        target: "voicesub.lifecycle",
        previous_pid = prev.pid,
        previous_started_utc_secs = prev.started_utc_secs,
        previous_version = %prev.version,
        "previous session exited without graceful shutdown"
    );
    deep_lifecycle_event(
        "shell",
        "abnormal_previous_exit",
        json!({
            "previous_pid": prev.pid,
            "previous_started_utc_secs": prev.started_utc_secs,
            "previous_version": prev.version,
        }),
    );
}

pub fn mark_session_running(project_root: &Path, version: &str) {
    let record = SessionLifecycleRecord {
        pid: process::id(),
        version: version.to_string(),
        started_utc_secs: utc_secs(),
        ended_utc_secs: None,
        state: SessionExitState::Running,
        reason: String::new(),
        detail: Value::Null,
    };
    write_record(&session_lifecycle_path(project_root), &record);
    deep_lifecycle_event(
        "shell",
        "session_start",
        json!({
            "pid": record.pid,
            "version": record.version,
            "started_utc_secs": record.started_utc_secs,
        }),
    );
}

pub fn install_lifecycle_hooks(project_root: &Path, version: &str) {
    inspect_previous_session(project_root);
    mark_session_running(project_root, version);
    install_panic_hook(project_root.to_path_buf(), version.to_string());
}

fn install_panic_hook(project_root: PathBuf, version: String) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let panic_payload = info.payload();
        let payload = panic_payload
            .downcast_ref::<&str>()
            .map(|text| (*text).to_string())
            .or_else(|| panic_payload.downcast_ref::<String>().cloned());
        let location = info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()));
        let fields = json!({
            "panic_payload": payload,
            "panic_location": location,
        });
        if is_deep_diagnostics_enabled() {
            pipeline_trace("desktop", "shell", "panic", fields.clone());
        }
        eprintln!("voicesub panic: {fields}");
        let path = session_lifecycle_path(&project_root);
        let mut record = read_session_lifecycle_record(&project_root).unwrap_or(SessionLifecycleRecord {
            pid: process::id(),
            version: version.clone(),
            started_utc_secs: utc_secs(),
            ended_utc_secs: None,
            state: SessionExitState::Running,
            reason: String::new(),
            detail: Value::Null,
        });
        record.state = SessionExitState::Panic;
        record.ended_utc_secs = Some(utc_secs());
        record.reason = "panic".into();
        record.detail = fields;
        write_record(&path, &record);
        default_hook(info);
    }));
}

pub fn log_shutdown_begin(reason: &str) {
    deep_lifecycle_event("shell", "shutdown_begin", json!({ "reason": reason }));
}

pub fn log_shutdown_step(step: &str, detail: Value) {
    deep_lifecycle_event(
        "shell",
        "shutdown_step",
        json!({ "step": step, "detail": detail }),
    );
}

pub fn complete_graceful_shutdown(project_root: &Path, reason: &str) {
    let path = session_lifecycle_path(project_root);
    let mut record = read_session_lifecycle_record(project_root).unwrap_or(SessionLifecycleRecord {
        pid: process::id(),
        version: String::new(),
        started_utc_secs: utc_secs(),
        ended_utc_secs: None,
        state: SessionExitState::Running,
        reason: String::new(),
        detail: Value::Null,
    });
    record.state = SessionExitState::Graceful;
    record.ended_utc_secs = Some(utc_secs());
    record.reason = reason.to_string();
    write_record(&path, &record);
    deep_lifecycle_event(
        "shell",
        "shutdown_complete",
        json!({
            "reason": reason,
            "ended_utc_secs": record.ended_utc_secs,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("voicesub-lifecycle-{name}-{}", process::id()))
    }

    fn cleanup(root: &Path) {
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn inspect_previous_session_warns_when_running_marker_left() {
        let root = temp_root("abnormal");
        cleanup(&root);
        let record = SessionLifecycleRecord {
            pid: 4242,
            version: "0.5.0".into(),
            started_utc_secs: 100,
            ended_utc_secs: None,
            state: SessionExitState::Running,
            reason: String::new(),
            detail: Value::Null,
        };
        write_record(&session_lifecycle_path(&root), &record);
        inspect_previous_session(&root);
        cleanup(&root);
    }

    #[test]
    fn complete_graceful_shutdown_marks_record() {
        let root = temp_root("graceful");
        cleanup(&root);
        mark_session_running(&root, "0.5.0-test");
        complete_graceful_shutdown(&root, "user_close");
        let saved = read_session_lifecycle_record(&root).expect("record");
        assert_eq!(saved.state, SessionExitState::Graceful);
        assert_eq!(saved.reason, "user_close");
        assert!(saved.ended_utc_secs.is_some());
        cleanup(&root);
    }
}
