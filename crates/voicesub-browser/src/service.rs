//! Port of SST `BrowserAsrService` — transport identity, stale ingress, control channel.

use std::sync::Arc;

use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex};
use tracing::warn;

use crate::operational_fsm::BrowserAsrOperationalFsm;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct BrowserAsrDiagnostics {
    pub worker_connected: bool,
    pub recognition_state: String,
    pub supervisor_state: String,
    pub desired_running: bool,
    pub degraded_reason: Option<String>,
    pub last_error: Option<String>,
    pub last_seen_at_ms: Option<u64>,
    pub session_id: Option<String>,
    pub generation_id: u64,
    pub client_segment_id: Option<String>,
    pub forced_final: bool,
    pub provider_name: Option<String>,
    pub active_recognition: bool,
    pub active_media_stream: bool,
    pub last_result_index: Option<u64>,
    pub browser_session_age_ms: Option<u64>,
    pub browser_cycle_pending: bool,
    pub browser_cycle_count: u64,
    pub browser_minimum_reconnect_suppressed_count: u64,
    pub browser_forced_final_on_interruption_count: u64,
    pub browser_restarts_count: u64,
    pub browser_no_speech_count: u64,
    pub browser_network_error_count: u64,
    pub duplicate_partial_suppressed: u64,
    pub duplicate_final_suppressed: u64,
    pub late_forced_final_suppressed: u64,
    pub mic_track_ready_state: Option<String>,
    pub mic_track_muted: bool,
    pub mic_rms: f64,
    pub mic_active_recent_ms: Option<u64>,
    pub get_user_media_count: u64,
    pub get_user_media_last_error: Option<String>,
    pub mic_stream_active: bool,
    pub media_tracks_stopped_count: u64,
    pub media_track_leak_guard_count: u64,
    pub browser_stale_events_ignored: u64,
    pub browser_worker_last_seen_age_ms: Option<u64>,
    pub browser_worker_generation: u64,
    pub operational_phase: String,
}

impl Default for BrowserAsrDiagnostics {
    fn default() -> Self {
        Self {
            worker_connected: false,
            recognition_state: "disconnected".into(),
            supervisor_state: "idle".into(),
            desired_running: false,
            degraded_reason: None,
            last_error: None,
            last_seen_at_ms: None,
            session_id: None,
            generation_id: 0,
            client_segment_id: None,
            forced_final: false,
            provider_name: None,
            active_recognition: false,
            active_media_stream: false,
            last_result_index: None,
            browser_session_age_ms: None,
            browser_cycle_pending: false,
            browser_cycle_count: 0,
            browser_minimum_reconnect_suppressed_count: 0,
            browser_forced_final_on_interruption_count: 0,
            browser_restarts_count: 0,
            browser_no_speech_count: 0,
            browser_network_error_count: 0,
            duplicate_partial_suppressed: 0,
            duplicate_final_suppressed: 0,
            late_forced_final_suppressed: 0,
            mic_track_ready_state: None,
            mic_track_muted: false,
            mic_rms: 0.0,
            mic_active_recent_ms: None,
            get_user_media_count: 0,
            get_user_media_last_error: None,
            mic_stream_active: false,
            media_tracks_stopped_count: 0,
            media_track_leak_guard_count: 0,
            browser_stale_events_ignored: 0,
            browser_worker_last_seen_age_ms: None,
            browser_worker_generation: 0,
            operational_phase: "idle".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IngestedAsrUpdate {
    pub partial: String,
    pub final_text: String,
    pub is_final: bool,
    pub source_lang: Option<String>,
    pub generation_id: u64,
    pub session_id: Option<String>,
    pub client_segment_id: Option<String>,
    pub forced_final: bool,
    pub transport_id: u64,
    pub worker_message_sequence: Option<u64>,
}

pub type IngestCallback = Arc<dyn Fn(IngestedAsrUpdate) + Send + Sync>;
pub type WorkerLifecycleCallback = Arc<dyn Fn() + Send + Sync>;
pub type StatusCallback = Arc<dyn Fn(Value) + Send + Sync>;

struct Inner {
    next_transport_id: u64,
    active_transport_id: u64,
    active_session_id: Option<String>,
    active_generation_id: u64,
    outbound: Option<mpsc::Sender<String>>,
    snapshot: BrowserAsrDiagnostics,
    fsm: BrowserAsrOperationalFsm,
}

pub struct BrowserAsrService {
    inner: Arc<Mutex<Inner>>,
    on_ingest: IngestCallback,
    on_worker_connected: Option<WorkerLifecycleCallback>,
    on_worker_disconnected: Option<WorkerLifecycleCallback>,
    on_status_update: Option<StatusCallback>,
}

impl BrowserAsrService {
    pub fn new(on_ingest: IngestCallback) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                next_transport_id: 0,
                active_transport_id: 0,
                active_session_id: None,
                active_generation_id: 0,
                outbound: None,
                snapshot: BrowserAsrDiagnostics::default(),
                fsm: BrowserAsrOperationalFsm::default(),
            })),
            on_ingest,
            on_worker_connected: None,
            on_worker_disconnected: None,
            on_status_update: None,
        }
    }

    pub fn with_hooks(
        on_ingest: IngestCallback,
        on_worker_connected: Option<WorkerLifecycleCallback>,
        on_worker_disconnected: Option<WorkerLifecycleCallback>,
        on_status_update: Option<StatusCallback>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                next_transport_id: 0,
                active_transport_id: 0,
                active_session_id: None,
                active_generation_id: 0,
                outbound: None,
                snapshot: BrowserAsrDiagnostics::default(),
                fsm: BrowserAsrOperationalFsm::default(),
            })),
            on_ingest,
            on_worker_connected,
            on_worker_disconnected,
            on_status_update,
        }
    }

    pub async fn register_connection(&self, outbound: mpsc::Sender<String>) -> u64 {
        let mut inner = self.inner.lock().await;
        inner.next_transport_id += 1;
        let transport_id = inner.next_transport_id;
        inner.active_transport_id = transport_id;
        inner.outbound = Some(outbound);
        inner.active_session_id = None;
        inner.active_generation_id = 0;
        inner.snapshot = BrowserAsrDiagnostics {
            worker_connected: true,
            recognition_state: "idle".into(),
            supervisor_state: "idle".into(),
            last_seen_at_ms: Some(now_ms()),
            ..BrowserAsrDiagnostics::default()
        };
        transport_id
    }

    pub async fn worker_connected(&self) {
        {
            let mut inner = self.inner.lock().await;
            inner.fsm.note_worker_connected();
        }
        if let Some(cb) = &self.on_worker_connected {
            cb();
        }
    }

    pub async fn disconnect(&self, transport_id: u64) {
        let should_notify = {
            let mut inner = self.inner.lock().await;
            if transport_id != inner.active_transport_id {
                return;
            }
            if inner.outbound.is_none() {
                return;
            }
            inner.outbound = None;
            inner.snapshot.worker_connected = false;
            inner.snapshot.recognition_state = "disconnected".into();
            inner.snapshot.supervisor_state = "idle".into();
            inner.snapshot.desired_running = false;
            inner.snapshot.active_recognition = false;
            inner.snapshot.active_media_stream = false;
            inner.snapshot.last_seen_at_ms = Some(now_ms());
            inner.fsm.note_worker_disconnected();
            true
        };
        if should_notify {
            if let Some(cb) = &self.on_worker_disconnected {
                cb();
            }
        }
    }

    pub async fn handle_status(&self, transport_id: u64, payload: &Value) -> bool {
        let accepted = self.accept_payload(transport_id, payload).await;
        if !accepted {
            return false;
        }
        self.apply_status_snapshot(transport_id, payload).await;
        if let Some(cb) = &self.on_status_update {
            let mut forwarded = payload.clone();
            if let Some(obj) = forwarded.as_object_mut() {
                obj.insert("basr_transport_id".into(), json!(transport_id));
            }
            cb(forwarded);
        }
        true
    }

    pub async fn handle_external_update(&self, transport_id: u64, payload: &Value) -> bool {
        if !self.accept_payload(transport_id, payload).await {
            return false;
        }
        let update = IngestedAsrUpdate {
            partial: payload_str(payload, "partial"),
            final_text: payload_str(payload, "final"),
            is_final: payload_bool(payload, "is_final"),
            source_lang: payload_opt_str(payload, "source_lang"),
            generation_id: payload_u64(payload, "generation_id"),
            session_id: payload_opt_str(payload, "session_id"),
            client_segment_id: payload_opt_str(payload, "client_segment_id"),
            forced_final: payload_bool(payload, "forced_final"),
            transport_id,
            worker_message_sequence: payload.get("worker_message_sequence").and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_i64().and_then(|n| u64::try_from(n).ok()))
            }),
        };
        {
            let mut inner = self.inner.lock().await;
            inner.fsm.note_ingest(update.is_final);
            inner.snapshot.last_seen_at_ms = Some(now_ms());
        }
        (self.on_ingest)(update);
        true
    }

    pub async fn send_control(&self, action: &str, reason: Option<&str>) -> bool {
        let (transport_id, outbound) = {
            let inner = self.inner.lock().await;
            (inner.active_transport_id, inner.outbound.clone())
        };
        let Some(tx) = outbound else {
            return false;
        };
        let payload = json!({
            "type": "browser_asr_control",
            "action": action.trim().to_ascii_lowercase(),
            "reason": reason.map(str::trim).filter(|s| !s.is_empty()),
            "issued_at_ms": now_ms(),
            "transport_id": transport_id,
        });
        let Ok(text) = serde_json::to_string(&payload) else {
            return false;
        };
        tx.send(text).await.is_ok()
    }

    pub async fn diagnostics(&self) -> BrowserAsrDiagnostics {
        let inner = self.inner.lock().await;
        let mut snap = inner.snapshot.clone();
        snap.browser_worker_generation = snap.generation_id;
        snap.operational_phase = inner.fsm.phase().as_str().into();
        if let Some(last_seen) = snap.last_seen_at_ms {
            snap.browser_worker_last_seen_age_ms = Some(now_ms().saturating_sub(last_seen));
        }
        snap
    }

    pub async fn has_active_transport(&self) -> bool {
        self.inner.lock().await.outbound.is_some()
    }

    async fn apply_status_snapshot(&self, transport_id: u64, payload: &Value) {
        let mut inner = self.inner.lock().await;
        inner.snapshot.worker_connected = true;
        inner.snapshot.recognition_state = payload
            .get("recognition_state")
            .and_then(|v| v.as_str())
            .unwrap_or(&inner.snapshot.recognition_state)
            .to_string();
        inner.snapshot.supervisor_state = payload
            .get("browser_supervisor_state")
            .or_else(|| payload.get("supervisor_state"))
            .and_then(|v| v.as_str())
            .unwrap_or(&inner.snapshot.supervisor_state)
            .to_string();
        inner.snapshot.desired_running = payload_bool(payload, "desired_running");
        inner.snapshot.degraded_reason = payload_opt_str(payload, "degraded_reason");
        inner.snapshot.last_error = payload_opt_str(payload, "last_error");
        inner.snapshot.last_seen_at_ms = Some(now_ms());
        inner.snapshot.provider_name = payload_opt_str(payload, "provider_name");
        inner.snapshot.generation_id = payload_u64(payload, "generation_id");
        inner.snapshot.session_id = payload_opt_str(payload, "session_id");
        inner.snapshot.client_segment_id = payload_opt_str(payload, "client_segment_id");
        inner.snapshot.forced_final = payload_bool(payload, "forced_final");
        inner.snapshot.active_recognition = payload_bool(payload, "active_recognition");
        inner.snapshot.active_media_stream = payload_bool(payload, "active_media_stream");
        inner.snapshot.last_result_index =
            payload.get("last_result_index").and_then(|v| v.as_u64());
        inner.snapshot.browser_session_age_ms = payload
            .get("browser_session_age_ms")
            .and_then(|v| v.as_u64());
        inner.snapshot.browser_cycle_pending = payload_bool(payload, "browser_cycle_pending");
        inner.snapshot.browser_cycle_count = payload_u64(payload, "browser_cycle_count");
        inner.snapshot.browser_minimum_reconnect_suppressed_count =
            payload_u64(payload, "browser_minimum_reconnect_suppressed_count");
        inner.snapshot.browser_forced_final_on_interruption_count =
            payload_u64(payload, "browser_forced_final_on_interruption_count");
        inner.snapshot.browser_restarts_count = payload_u64(payload, "restart_count");
        inner.snapshot.browser_no_speech_count = payload_u64(payload, "no_speech_count");
        inner.snapshot.browser_network_error_count = payload_u64(payload, "network_error_count");
        inner.snapshot.duplicate_partial_suppressed =
            payload_u64(payload, "duplicate_partial_suppressed");
        inner.snapshot.duplicate_final_suppressed =
            payload_u64(payload, "duplicate_final_suppressed");
        inner.snapshot.late_forced_final_suppressed =
            payload_u64(payload, "late_forced_final_suppressed");
        inner.snapshot.mic_track_ready_state = payload_opt_str(payload, "mic_track_ready_state");
        inner.snapshot.mic_track_muted = payload_bool(payload, "mic_track_muted");
        inner.snapshot.mic_rms = payload
            .get("mic_rms")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        inner.snapshot.mic_active_recent_ms =
            payload.get("mic_active_recent_ms").and_then(|v| v.as_u64());
        inner.snapshot.get_user_media_count = payload_u64(payload, "get_user_media_count");
        inner.snapshot.get_user_media_last_error =
            payload_opt_str(payload, "get_user_media_last_error");
        inner.snapshot.mic_stream_active = payload_bool(payload, "mic_stream_active");
        inner.snapshot.media_tracks_stopped_count =
            payload_u64(payload, "media_tracks_stopped_count");
        inner.snapshot.media_track_leak_guard_count =
            payload_u64(payload, "media_track_leak_guard_count");
        let degraded = inner.snapshot.degraded_reason.clone();
        inner.fsm.note_status_aggregate(true, degraded.as_deref());
        let _ = transport_id;
    }

    async fn accept_payload(&self, transport_id: u64, payload: &Value) -> bool {
        let mut inner = self.inner.lock().await;
        if transport_id != inner.active_transport_id {
            inner.snapshot.browser_stale_events_ignored += 1;
            warn!(
                transport_id,
                active = inner.active_transport_id,
                "wrong_transport"
            );
            return false;
        }
        let session_id = payload_opt_str(payload, "session_id");
        let generation_id = payload_u64(payload, "generation_id");

        if let (Some(sid), Some(active_sid)) = (&session_id, &inner.active_session_id) {
            if sid != active_sid {
                if generation_id > 0 && generation_id <= inner.active_generation_id {
                    inner.snapshot.browser_stale_events_ignored += 1;
                    return false;
                }
                inner.active_session_id = session_id.clone();
                inner.active_generation_id = generation_id;
                return true;
            }
        }
        if session_id.is_some() && inner.active_session_id.is_none() {
            inner.active_session_id = session_id;
        }
        if generation_id > 0 && generation_id < inner.active_generation_id {
            inner.snapshot.browser_stale_events_ignored += 1;
            return false;
        }
        if generation_id > 0 {
            inner.active_generation_id = generation_id;
        }
        true
    }
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn payload_str(payload: &Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn payload_opt_str(payload: &Value, key: &str) -> Option<String> {
    let value = payload.get(key).and_then(|v| v.as_str())?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn payload_bool(payload: &Value, key: &str) -> bool {
    payload.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn payload_u64(payload: &Value, key: &str) -> u64 {
    payload
        .get(key)
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|n| n.max(0) as u64)))
        .unwrap_or(0)
}
