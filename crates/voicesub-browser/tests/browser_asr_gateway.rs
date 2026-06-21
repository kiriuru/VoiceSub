use std::sync::{Arc, Mutex};

use serde_json::json;
use voicesub_browser::{BrowserAsrGateway, StructuredLogFn, structured_log_from_runtime_logger};
use voicesub_logging::{StructuredRuntimeLogger, set_config_full_logging_enabled};

#[derive(Debug, Clone)]
struct LogRecord {
    event: String,
}

struct RecordingStructuredLog {
    records: Arc<Mutex<Vec<LogRecord>>>,
}

impl RecordingStructuredLog {
    fn new() -> Self {
        Self {
            records: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn structured_log_fn(&self) -> StructuredLogFn {
        let records = self.records.clone();
        Arc::new(move |_channel, event, _fields| {
            records.lock().unwrap().push(LogRecord {
                event: event.into(),
            });
        })
    }

    fn events(&self) -> Vec<String> {
        self.records
            .lock()
            .unwrap()
            .iter()
            .map(|record| record.event.clone())
            .collect()
    }
}

#[test]
fn tracks_connection_and_logs_final_not_partial() {
    let logger = RecordingStructuredLog::new();
    let mut gateway = BrowserAsrGateway::new(Some(logger.structured_log_fn()));

    gateway.worker_connected();
    gateway.update_status(&json!({
        "reason": "recognition-started",
        "desired_running": true,
        "recognition_running": true,
        "recognition_state": "running",
        "client_segment_id": "browser-seg-3",
        "forced_final": true,
        "provider_name": "browser_google",
        "active_recognition": true,
        "degraded_reason": "document_hidden",
        "visibility_state": "hidden",
        "rearm_count": 2,
    }));
    gateway.note_partial(7, Some("en"), Some(11));
    gateway.note_final(12, Some("en"), Some(12), false);

    let events = logger.events();
    assert!(events.contains(&"browser_worker_connected".to_string()));
    assert!(events.contains(&"browser_recognition_started".to_string()));
    assert!(events.contains(&"browser_degraded".to_string()));
    assert!(
        !events
            .iter()
            .any(|event| event == "browser_external_partial")
    );
    assert!(events.contains(&"browser_external_final".to_string()));

    gateway.worker_disconnected();
    let diagnostics = gateway.diagnostics();
    assert!(!diagnostics.worker_connected);
    assert_eq!(
        diagnostics.recognition_state.as_deref(),
        Some("disconnected")
    );
}

#[test]
fn note_partial_does_not_emit_structured_events() {
    let logger = RecordingStructuredLog::new();
    let mut gateway = BrowserAsrGateway::new(Some(logger.structured_log_fn()));
    gateway.worker_connected();
    logger.records.lock().unwrap().clear();
    for index in 0..40 {
        gateway.note_partial(5, Some("en"), Some(index));
    }
    assert!(
        !logger
            .events()
            .iter()
            .any(|event| event == "browser_external_partial")
    );
}

#[test]
fn samples_routine_recognition_started_events() {
    let logger = RecordingStructuredLog::new();
    let mut gateway = BrowserAsrGateway::new(Some(logger.structured_log_fn()));
    gateway.worker_connected();
    logger.records.lock().unwrap().clear();

    gateway.update_status(&json!({
        "reason": "recognition-started",
        "desired_running": true,
        "recognition_running": true,
        "recognition_state": "running",
        "visibility_state": "visible",
        "rearm_count": 24,
    }));
    assert!(
        !logger
            .events()
            .contains(&"browser_worker_status".to_string())
    );
    assert!(
        !logger
            .events()
            .contains(&"browser_recognition_started".to_string())
    );

    logger.records.lock().unwrap().clear();
    gateway.update_status(&json!({
        "reason": "recognition-started",
        "desired_running": true,
        "recognition_running": true,
        "recognition_state": "running",
        "visibility_state": "visible",
        "rearm_count": 25,
    }));
    assert!(
        logger
            .events()
            .contains(&"browser_recognition_started".to_string())
    );
}

#[test]
fn identical_status_uses_heartbeat_instead_of_full_snapshot() {
    let logger = RecordingStructuredLog::new();
    let mut gateway = BrowserAsrGateway::new(Some(logger.structured_log_fn()));
    gateway.worker_connected();
    gateway.update_status(&json!({
        "reason": "heartbeat",
        "desired_running": true,
        "recognition_running": true,
        "recognition_state": "running",
        "generation_id": 7,
        "browser_cycle_count": 2,
        "last_partial_age_ms": 30,
    }));

    logger.records.lock().unwrap().clear();
    gateway.set_last_heartbeat_ago_for_test(20_000);
    gateway.update_status(&json!({
        "reason": "heartbeat",
        "desired_running": true,
        "recognition_running": true,
        "recognition_state": "running",
        "generation_id": 7,
        "browser_cycle_count": 2,
        "last_partial_age_ms": 95,
    }));

    let events = logger.events();
    assert!(!events.contains(&"browser_worker_status".to_string()));
    assert!(events.contains(&"browser_worker_heartbeat".to_string()));
}

#[test]
fn detail_only_worker_status_respects_interval() {
    let logger = RecordingStructuredLog::new();
    let mut gateway = BrowserAsrGateway::new(Some(logger.structured_log_fn()));
    gateway.worker_connected();
    logger.records.lock().unwrap().clear();

    gateway.update_status(&json!({
        "reason": "result",
        "desired_running": true,
        "recognition_running": true,
        "recognition_state": "running",
        "browser_supervisor_state": "running",
        "mic_rms": 0.01,
        "last_result_index": 1,
    }));
    gateway.update_status(&json!({
        "reason": "duplicate-partial",
        "mic_rms": 0.55,
        "last_result_index": 99,
    }));
    let status_events = logger
        .events()
        .iter()
        .filter(|event| *event == "browser_worker_status")
        .count();
    assert_eq!(status_events, 1);

    logger.records.lock().unwrap().clear();
    gateway.set_detail_only_interval_ms_for_test(0);
    gateway.update_status(&json!({
        "reason": "result",
        "mic_rms": 0.02,
        "last_result_index": 2,
    }));
    gateway.update_status(&json!({
        "reason": "result",
        "mic_rms": 0.03,
        "last_result_index": 3,
    }));
    let status_events = logger
        .events()
        .iter()
        .filter(|event| *event == "browser_worker_status")
        .count();
    assert_eq!(status_events, 2);
}

#[test]
fn maps_overlap_telemetry_events() {
    let logger = RecordingStructuredLog::new();
    let mut gateway = BrowserAsrGateway::new(Some(logger.structured_log_fn()));
    gateway.worker_connected();
    logger.records.lock().unwrap().clear();

    for (reason, expected) in [
        ("overlap-handoff", "browser_overlap_handoff"),
        ("overlap-buddy-ended", "browser_overlap_buddy_ended"),
        ("overlap-buddy-error", "browser_overlap_buddy_error"),
        (
            "overlap-buddy-ghost-recovered",
            "browser_overlap_buddy_ghost_recovered",
        ),
    ] {
        logger.records.lock().unwrap().clear();
        gateway.update_status(&json!({
            "reason": reason,
            "desired_running": true,
            "recognition_running": true,
            "recognition_state": "running",
            "overlap_active": true,
            "overlap_active_slot": 1,
            "overlap_buddy_slot": 0,
            "overlap_buddy_listening": true,
            "overlap_prestarted": true,
        }));
        assert!(
            logger.events().contains(&expected.to_string()),
            "reason={reason} expected {expected}"
        );
    }
}

#[test]
fn stores_overlap_fields_in_diagnostics() {
    let logger = RecordingStructuredLog::new();
    let mut gateway = BrowserAsrGateway::new(Some(logger.structured_log_fn()));
    gateway.worker_connected();
    gateway.update_status(&json!({
        "reason": "result",
        "desired_running": true,
        "recognition_running": true,
        "overlap_mode_desired": true,
        "overlap_active": true,
        "overlap_active_slot": 0,
        "overlap_buddy_slot": 1,
        "overlap_prestarted": true,
        "overlap_active_listening": true,
        "overlap_buddy_listening": false,
        "overlap_prestart_timer_armed": true,
    }));

    let diagnostics = gateway.diagnostics();
    assert!(diagnostics.overlap_mode_desired);
    assert!(diagnostics.overlap_active);
    assert_eq!(diagnostics.overlap_active_slot, Some(0));
    assert_eq!(diagnostics.overlap_buddy_slot, Some(1));
    assert!(diagnostics.overlap_prestarted);
    assert!(diagnostics.overlap_active_listening);
    assert!(!diagnostics.overlap_buddy_listening);
    assert!(diagnostics.overlap_prestart_timer_armed);
}

#[test]
fn structured_log_from_runtime_logger_writes_browser_channel() {
    let dir = std::env::temp_dir().join(format!("voicesub-browser-gateway-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    set_config_full_logging_enabled(true);
    let runtime_logger = Arc::new(StructuredRuntimeLogger::new(&dir));
    let structured = structured_log_from_runtime_logger(runtime_logger.clone());
    let mut gateway = BrowserAsrGateway::new(Some(structured));
    gateway.worker_connected();
    gateway.note_final(4, Some("en"), Some(1), false);

    let joined = std::fs::read_to_string(runtime_logger.log_path()).unwrap_or_default();
    assert!(joined.contains("browser_worker_connected"));
    assert!(joined.contains("browser_external_final"));
    assert!(joined.contains("Browser Asr Gateway"));

    set_config_full_logging_enabled(false);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn reset_ingest_session_preserves_worker_connection() {
    let mut gateway = BrowserAsrGateway::new(None);
    gateway.worker_connected();
    gateway.update_status(&json!({
        "reason": "recognition-started",
        "generation_id": 3,
        "session_id": "browser-worker-test",
        "client_segment_id": "seg-1",
    }));
    gateway.note_final(2, Some("ru"), Some(3), true);
    gateway.note_stale_worker_event_ignored();

    let before = gateway.diagnostics();
    assert!(before.worker_connected);
    assert_eq!(before.generation_id, 3);
    assert_eq!(before.stale_worker_events_ignored, 1);
    assert_eq!(before.session_id.as_deref(), Some("browser-worker-test"));

    gateway.reset_ingest_session();
    let after = gateway.diagnostics();
    assert!(after.worker_connected);
    assert_eq!(after.generation_id, 0);
    assert_eq!(after.stale_worker_events_ignored, 0);
    assert!(after.session_id.is_none());
    assert!(!after.forced_final);
}
