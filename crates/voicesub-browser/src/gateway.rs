//! Structured runtime logging for browser ASR (SST `BrowserAsrGateway` port).

use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Value, json};

use crate::trace::{BROWSER_LOG_CHANNEL, BrowserAsrLog, StructuredLogFn};

const ROUTINE_RESTART_EVENTS: &[&str] = &[
    "browser_recognition_started",
    "browser_onend",
    "browser_rearm_scheduled",
    "browser_rearm_executed",
    "recognition_onstart",
    "recognition_onend",
];
const NOISY_ERROR_TYPES: &[&str] = &["no-speech"];
const ROUTINE_LOG_SAMPLE_EVERY: u64 = 25;
const ROUTINE_LOG_VERBOSE_LIMIT: u64 = 3;
const STATUS_HEARTBEAT_INTERVAL_MS: u64 = 15_000;
const DETAIL_ONLY_STATUS_LOG_MIN_INTERVAL_MS: u64 = 3_600_000;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GatewayDiagnostics {
    pub worker_connected: bool,
    pub browser_mode: Option<String>,
    pub start_mode: Option<String>,
    pub session_id: Option<String>,
    pub generation_id: u64,
    pub client_segment_id: Option<String>,
    pub desired_running: bool,
    pub pending_start: bool,
    pub recognition_running: bool,
    pub recognition_state: Option<String>,
    pub supervisor_state: Option<String>,
    pub effective_continuous_mode: Option<String>,
    pub recognition_continuous: bool,
    pub websocket_ready: bool,
    pub forced_final: bool,
    pub provider_name: Option<String>,
    pub active_recognition: bool,
    pub active_media_stream: bool,
    pub last_result_index: Option<u64>,
    pub last_result_at_ms: Option<u64>,
    pub last_partial_age_ms: Option<u64>,
    pub last_final_age_ms: Option<u64>,
    pub last_session_started_at_ms: Option<u64>,
    pub last_session_ended_at_ms: Option<u64>,
    pub browser_session_age_ms: Option<u64>,
    pub browser_cycle_pending: bool,
    pub browser_cycle_count: u64,
    pub browser_minimum_reconnect_suppressed_count: u64,
    pub browser_forced_final_on_interruption_count: u64,
    pub last_error: Option<String>,
    pub error_type: Option<String>,
    pub rearm_count: u64,
    pub restart_count: u64,
    pub no_speech_count: u64,
    pub network_error_count: u64,
    pub watchdog_rearm_count: u64,
    pub duplicate_partial_suppressed: u64,
    pub duplicate_final_suppressed: u64,
    pub late_forced_final_suppressed: u64,
    pub degraded_reason: Option<String>,
    pub visibility_state: Option<String>,
    pub mic_track_ready_state: Option<String>,
    pub mic_track_muted: bool,
    pub mic_rms: Option<f64>,
    pub mic_active_recent_ms: Option<u64>,
    pub last_mic_activity_at: Option<u64>,
    pub get_user_media_count: u64,
    pub get_user_media_last_error: Option<String>,
    pub mic_stream_active: bool,
    pub media_tracks_stopped_count: u64,
    pub media_track_leak_guard_count: u64,
    pub last_status_reason: Option<String>,
    pub last_rearm_delay_ms: Option<u64>,
    pub stopping_since_ms: Option<u64>,
    pub last_seen_at_ms: Option<u64>,
    pub stale_worker_events_ignored: u64,
    pub overlap_mode_desired: bool,
    pub overlap_active: bool,
    pub overlap_active_slot: Option<u64>,
    pub overlap_buddy_slot: Option<u64>,
    pub overlap_prestarted: bool,
    pub overlap_active_listening: bool,
    pub overlap_buddy_listening: bool,
    pub overlap_prestart_timer_armed: bool,
}

pub struct BrowserAsrGateway {
    state: GatewayDiagnostics,
    log: BrowserAsrLog,
    last_status_heartbeat: Instant,
    last_browser_worker_status_log: Instant,
    heartbeat_counter_baseline: CounterSnapshot,
    detail_only_status_log_min_interval_ms: u64,
}

impl BrowserAsrGateway {
    pub fn new(structured_log: Option<StructuredLogFn>) -> Self {
        let state = GatewayDiagnostics::default();
        let heartbeat_counter_baseline = counter_snapshot(&state);
        Self {
            state,
            log: BrowserAsrLog::new(structured_log),
            last_status_heartbeat: Instant::now(),
            last_browser_worker_status_log: Instant::now(),
            heartbeat_counter_baseline,
            detail_only_status_log_min_interval_ms: DETAIL_ONLY_STATUS_LOG_MIN_INTERVAL_MS,
        }
    }

    pub fn reset_ingest_session(&mut self) {
        self.state.stale_worker_events_ignored = 0;
        self.state.session_id = None;
        self.state.generation_id = 0;
        self.state.client_segment_id = None;
        self.state.forced_final = false;
        self.state.last_partial_age_ms = None;
        self.state.last_final_age_ms = None;
        self.heartbeat_counter_baseline = counter_snapshot(&self.state);
    }

    pub fn reset(&mut self) {
        self.state = GatewayDiagnostics::default();
        self.last_status_heartbeat = Instant::now();
        self.last_browser_worker_status_log = Instant::now();
        self.heartbeat_counter_baseline = counter_snapshot(&self.state);
    }

    pub fn worker_connected(&mut self) {
        self.state.worker_connected = true;
        self.state.browser_mode = Some("browser_google".into());
        self.state.provider_name = Some("browser_google".into());
        self.state.last_error = None;
        self.log_event(
            "browser_worker_connected",
            json!({
                "worker_connected": true,
                "browser_mode": self.state.browser_mode,
                "recognition_state": self.state.recognition_state,
                "websocket_ready": self.state.websocket_ready,
            }),
        );
    }

    pub fn worker_disconnected(&mut self) {
        let previous = self.state.clone();
        self.state.worker_connected = false;
        self.state.recognition_running = false;
        self.state.recognition_state = Some("disconnected".into());
        self.state.websocket_ready = false;
        self.log_event(
            "browser_worker_disconnected",
            json!({
                "worker_connected": false,
                "browser_mode": self.state.browser_mode,
                "recognition_state": "disconnected",
                "desired_running": previous.desired_running,
                "last_error": previous.last_error,
                "degraded_reason": previous.degraded_reason,
            }),
        );
    }

    pub fn note_partial(
        &mut self,
        text_len: usize,
        source_lang: Option<&str>,
        sequence: Option<u64>,
    ) {
        let _ = (text_len, source_lang, sequence);
        self.state.last_partial_age_ms = Some(0);
    }

    pub fn note_final(
        &mut self,
        text_len: usize,
        source_lang: Option<&str>,
        sequence: Option<u64>,
        forced_final: bool,
    ) {
        self.state.forced_final = forced_final;
        self.state.last_final_age_ms = Some(0);
        self.log_event(
            "browser_external_final",
            json!({
                "worker_connected": self.state.worker_connected,
                "sequence": sequence,
                "text_len": text_len,
                "source_lang": source_lang,
                "is_final": true,
                "forced_final": forced_final,
            }),
        );
    }

    pub fn note_stale_worker_event_ignored(&mut self) {
        self.state.stale_worker_events_ignored =
            self.state.stale_worker_events_ignored.saturating_add(1);
    }

    pub fn update_status(&mut self, payload: &Value) {
        let Some(obj) = payload.as_object() else {
            return;
        };
        let previous = self.state.clone();
        apply_status_payload(&mut self.state, obj);

        let reason = self.state.last_status_reason.clone();
        let mapped_event = map_reason_to_event(reason.as_deref());

        if should_log_status_snapshot(
            &previous,
            &self.state,
            reason.as_deref(),
            mapped_event,
            self.last_browser_worker_status_log,
            self.detail_only_interval_ms(),
        ) {
            self.log_event(
                "browser_worker_status",
                structured_status_log_summary(&self.state, reason.as_deref()),
            );
            self.last_browser_worker_status_log = Instant::now();
            self.mark_status_activity();
        } else if should_log_status_heartbeat(&self.state, self.last_status_heartbeat) {
            self.log_event(
                "browser_worker_heartbeat",
                heartbeat_payload(&self.state, &self.heartbeat_counter_baseline),
            );
            self.mark_status_activity();
        }

        if let Some(event) = mapped_event
            && should_log_mapped_event(event, &self.state)
        {
            self.log_event(event, structured_mapped_event_log_summary(&self.state));
        }

        if self.state.last_error.is_some()
            && (matches!(
                reason.as_deref(),
                Some("recognition-error")
                    | Some("terminal-error")
                    | Some("microphone-permission-failed")
            ) || previous.last_error != self.state.last_error)
            && should_log_error_event(&self.state)
        {
            self.log_event(
                "browser_error",
                json!({
                    "error": self.state.last_error,
                    "error_type": self.state.error_type,
                    "browser_mode": self.state.browser_mode,
                    "start_mode": self.state.start_mode,
                    "recognition_state": self.state.recognition_state,
                    "visibility_state": self.state.visibility_state,
                    "worker_connected": self.state.worker_connected,
                }),
            );
        }

        if self.state.degraded_reason.is_some()
            && previous.degraded_reason != self.state.degraded_reason
        {
            self.log_event(
                "browser_degraded",
                json!({
                    "browser_mode": self.state.browser_mode,
                    "start_mode": self.state.start_mode,
                    "degraded_reason": self.state.degraded_reason,
                    "desired_running": self.state.desired_running,
                    "recognition_state": self.state.recognition_state,
                    "visibility_state": self.state.visibility_state,
                    "worker_connected": self.state.worker_connected,
                }),
            );
        }
    }

    pub fn diagnostics(&self) -> GatewayDiagnostics {
        self.state.clone()
    }

    #[doc(hidden)]
    pub fn set_last_heartbeat_ago_for_test(&mut self, ago_ms: u64) {
        self.last_status_heartbeat = Instant::now() - Duration::from_millis(ago_ms);
    }

    #[doc(hidden)]
    pub fn set_detail_only_interval_ms_for_test(&mut self, ms: u64) {
        self.detail_only_status_log_min_interval_ms = ms;
    }

    fn detail_only_interval_ms(&self) -> u64 {
        self.detail_only_status_log_min_interval_ms
    }

    fn mark_status_activity(&mut self) {
        self.last_status_heartbeat = Instant::now();
        self.heartbeat_counter_baseline = counter_snapshot(&self.state);
    }

    fn log_event(&self, event: &str, fields: Value) {
        let _ = BROWSER_LOG_CHANNEL;
        self.log.emit(event, fields);
    }
}

#[derive(Debug, Clone, Default)]
struct CounterSnapshot {
    rearm_count: u64,
    restart_count: u64,
    watchdog_rearm_count: u64,
    no_speech_count: u64,
    network_error_count: u64,
    duplicate_partial_suppressed: u64,
    duplicate_final_suppressed: u64,
    late_forced_final_suppressed: u64,
    stale_worker_events_ignored: u64,
}

fn counter_snapshot(state: &GatewayDiagnostics) -> CounterSnapshot {
    CounterSnapshot {
        rearm_count: state.rearm_count,
        restart_count: state.restart_count,
        watchdog_rearm_count: state.watchdog_rearm_count,
        no_speech_count: state.no_speech_count,
        network_error_count: state.network_error_count,
        duplicate_partial_suppressed: state.duplicate_partial_suppressed,
        duplicate_final_suppressed: state.duplicate_final_suppressed,
        late_forced_final_suppressed: state.late_forced_final_suppressed,
        stale_worker_events_ignored: state.stale_worker_events_ignored,
    }
}

fn apply_status_payload(state: &mut GatewayDiagnostics, payload: &serde_json::Map<String, Value>) {
    apply_bool(state, payload, "desired_running", |s, v| {
        s.desired_running = v
    });
    apply_bool(state, payload, "pending_start", |s, v| s.pending_start = v);
    apply_bool(state, payload, "recognition_running", |s, v| {
        s.recognition_running = v
    });
    apply_bool(state, payload, "recognition_continuous", |s, v| {
        s.recognition_continuous = v
    });
    apply_bool(state, payload, "websocket_ready", |s, v| {
        s.websocket_ready = v
    });
    apply_bool(state, payload, "forced_final", |s, v| s.forced_final = v);
    apply_bool(state, payload, "active_recognition", |s, v| {
        s.active_recognition = v
    });
    apply_bool(state, payload, "active_media_stream", |s, v| {
        s.active_media_stream = v
    });
    apply_bool(state, payload, "browser_cycle_pending", |s, v| {
        s.browser_cycle_pending = v
    });
    apply_bool(state, payload, "mic_stream_active", |s, v| {
        s.mic_stream_active = v
    });
    apply_bool(state, payload, "mic_track_muted", |s, v| {
        s.mic_track_muted = v
    });

    apply_str(state, payload, "browser_mode", |s, v| s.browser_mode = v);
    apply_str(state, payload, "start_mode", |s, v| s.start_mode = v);
    apply_str(state, payload, "recognition_state", |s, v| {
        s.recognition_state = v
    });
    apply_str(state, payload, "browser_supervisor_state", |s, v| {
        s.supervisor_state = v
    });
    apply_str(state, payload, "supervisor_state", |s, v| {
        s.supervisor_state = v
    });
    apply_str(state, payload, "effective_continuous_mode", |s, v| {
        s.effective_continuous_mode = v
    });
    apply_str(state, payload, "provider_name", |s, v| s.provider_name = v);
    apply_str(state, payload, "last_error", |s, v| s.last_error = v);
    apply_str(state, payload, "get_user_media_last_error", |s, v| {
        s.get_user_media_last_error = v
    });
    apply_str(state, payload, "degraded_reason", |s, v| {
        s.degraded_reason = v
    });
    apply_str(state, payload, "visibility_state", |s, v| {
        s.visibility_state = v
    });
    apply_str(state, payload, "error_type", |s, v| s.error_type = v);
    apply_str(state, payload, "session_id", |s, v| s.session_id = v);
    apply_str(state, payload, "client_segment_id", |s, v| {
        s.client_segment_id = v
    });
    apply_str(state, payload, "mic_track_ready_state", |s, v| {
        s.mic_track_ready_state = v
    });
    apply_str(state, payload, "reason", |s, v| s.last_status_reason = v);

    apply_u64(state, payload, "rearm_count", |s, v| s.rearm_count = v);
    apply_u64(state, payload, "restart_count", |s, v| s.restart_count = v);
    apply_u64(state, payload, "watchdog_rearm_count", |s, v| {
        s.watchdog_rearm_count = v
    });
    apply_u64(state, payload, "rearm_delay_ms", |s, v| {
        s.last_rearm_delay_ms = Some(v)
    });
    apply_u64(state, payload, "last_partial_age_ms", |s, v| {
        s.last_partial_age_ms = Some(v)
    });
    apply_u64(state, payload, "last_final_age_ms", |s, v| {
        s.last_final_age_ms = Some(v)
    });
    apply_u64(state, payload, "generation_id", |s, v| s.generation_id = v);
    apply_u64(state, payload, "no_speech_count", |s, v| {
        s.no_speech_count = v
    });
    apply_u64(state, payload, "network_error_count", |s, v| {
        s.network_error_count = v
    });
    apply_u64(state, payload, "last_result_index", |s, v| {
        s.last_result_index = Some(v)
    });
    apply_u64(state, payload, "last_result_at_ms", |s, v| {
        s.last_result_at_ms = Some(v)
    });
    apply_u64(state, payload, "last_session_started_at_ms", |s, v| {
        s.last_session_started_at_ms = Some(v)
    });
    apply_u64(state, payload, "last_session_ended_at_ms", |s, v| {
        s.last_session_ended_at_ms = Some(v)
    });
    apply_u64(state, payload, "browser_session_age_ms", |s, v| {
        s.browser_session_age_ms = Some(v)
    });
    apply_u64(state, payload, "browser_cycle_count", |s, v| {
        s.browser_cycle_count = v
    });
    apply_u64(
        state,
        payload,
        "browser_minimum_reconnect_suppressed_count",
        |s, v| s.browser_minimum_reconnect_suppressed_count = v,
    );
    apply_u64(
        state,
        payload,
        "browser_forced_final_on_interruption_count",
        |s, v| s.browser_forced_final_on_interruption_count = v,
    );
    apply_u64(state, payload, "duplicate_partial_suppressed", |s, v| {
        s.duplicate_partial_suppressed = v
    });
    apply_u64(state, payload, "duplicate_final_suppressed", |s, v| {
        s.duplicate_final_suppressed = v
    });
    apply_u64(state, payload, "late_forced_final_suppressed", |s, v| {
        s.late_forced_final_suppressed = v
    });
    apply_u64(state, payload, "mic_active_recent_ms", |s, v| {
        s.mic_active_recent_ms = Some(v)
    });
    apply_u64(state, payload, "last_mic_activity_at", |s, v| {
        s.last_mic_activity_at = Some(v)
    });
    apply_u64(state, payload, "get_user_media_count", |s, v| {
        s.get_user_media_count = v
    });
    apply_u64(state, payload, "media_tracks_stopped_count", |s, v| {
        s.media_tracks_stopped_count = v
    });
    apply_u64(state, payload, "media_track_leak_guard_count", |s, v| {
        s.media_track_leak_guard_count = v
    });
    apply_u64(state, payload, "stopping_since_ms", |s, v| {
        s.stopping_since_ms = Some(v)
    });
    apply_u64(state, payload, "last_seen_at_ms", |s, v| {
        s.last_seen_at_ms = Some(v)
    });
    apply_u64(state, payload, "stale_worker_events_ignored", |s, v| {
        s.stale_worker_events_ignored = v
    });

    if let Some(value) = payload.get("mic_rms").and_then(|v| v.as_f64()) {
        state.mic_rms = Some(value.max(0.0));
    }

    apply_bool(state, payload, "overlap_mode_desired", |s, v| {
        s.overlap_mode_desired = v
    });
    apply_bool(state, payload, "overlap_active", |s, v| {
        s.overlap_active = v
    });
    apply_bool(state, payload, "overlap_prestarted", |s, v| {
        s.overlap_prestarted = v
    });
    apply_bool(state, payload, "overlap_active_listening", |s, v| {
        s.overlap_active_listening = v
    });
    apply_bool(state, payload, "overlap_buddy_listening", |s, v| {
        s.overlap_buddy_listening = v
    });
    apply_bool(state, payload, "overlap_prestart_timer_armed", |s, v| {
        s.overlap_prestart_timer_armed = v
    });
    apply_opt_u64(state, payload, "overlap_active_slot", |s, v| {
        s.overlap_active_slot = v
    });
    apply_opt_u64(state, payload, "overlap_buddy_slot", |s, v| {
        s.overlap_buddy_slot = v
    });

    if let Some(mode) = state.browser_mode.as_deref()
        && mode == "browser_google"
    {
        state
            .provider_name
            .get_or_insert_with(|| "browser_google".into());
    }
}

fn apply_bool<F>(
    state: &mut GatewayDiagnostics,
    payload: &serde_json::Map<String, Value>,
    key: &str,
    set: F,
) where
    F: FnOnce(&mut GatewayDiagnostics, bool),
{
    if let Some(value) = payload.get(key).and_then(|v| v.as_bool()) {
        set(state, value);
    }
}

fn apply_str<F>(
    state: &mut GatewayDiagnostics,
    payload: &serde_json::Map<String, Value>,
    key: &str,
    set: F,
) where
    F: FnOnce(&mut GatewayDiagnostics, Option<String>),
{
    if !payload.contains_key(key) {
        return;
    }
    let normalized = payload
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if key == "browser_mode" {
        let mode = normalized
            .as_deref()
            .filter(|m| *m == "browser_google")
            .map(str::to_string);
        set(state, mode);
        return;
    }
    set(state, normalized);
}

fn apply_u64<F>(
    state: &mut GatewayDiagnostics,
    payload: &serde_json::Map<String, Value>,
    key: &str,
    set: F,
) where
    F: FnOnce(&mut GatewayDiagnostics, u64),
{
    if let Some(value) = payload.get(key) {
        let parsed = value
            .as_u64()
            .or_else(|| value.as_i64().map(|n| n.max(0) as u64))
            .unwrap_or(0);
        set(state, parsed);
    }
}

fn apply_opt_u64<F>(
    state: &mut GatewayDiagnostics,
    payload: &serde_json::Map<String, Value>,
    key: &str,
    set: F,
) where
    F: FnOnce(&mut GatewayDiagnostics, Option<u64>),
{
    if !payload.contains_key(key) {
        return;
    }
    let parsed = payload.get(key).and_then(|value| {
        if value.is_null() {
            return None;
        }
        value
            .as_u64()
            .or_else(|| value.as_i64().map(|n| n.max(0) as u64))
    });
    set(state, parsed);
}

fn map_reason_to_event(reason: Option<&str>) -> Option<&'static str> {
    match reason.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "start-requested" => Some("browser_recognition_start_requested"),
        "recognition-started" => Some("browser_recognition_started"),
        "recognition-ended" => Some("browser_onend"),
        "overlap-handoff" => Some("browser_overlap_handoff"),
        "overlap-buddy-ended" => Some("browser_overlap_buddy_ended"),
        "overlap-buddy-error" => Some("browser_overlap_buddy_error"),
        "overlap-buddy-ghost-recovered" => Some("browser_overlap_buddy_ghost_recovered"),
        "recognition-error" => Some("browser_onerror"),
        "restart-scheduled" => Some("browser_rearm_scheduled"),
        "restart-executed" => Some("browser_rearm_executed"),
        "watchdog-rearm" => Some("browser_watchdog_rearm"),
        "visibility" => Some("browser_visibility_changed"),
        "user-stop" => None,
        "audio-track-permission-requested" => Some("audio_track_permission_requested"),
        "audio-track-permission-granted" => Some("audio_track_permission_granted"),
        "audio-track-permission-denied" => Some("audio_track_permission_denied"),
        "audio-track-opened" => Some("audio_track_opened"),
        "audio-track-reused" => Some("audio_track_reused"),
        "audio-track-ended" => Some("audio_track_ended"),
        "audio-track-muted" => Some("audio_track_muted"),
        "audio-track-unmuted" => Some("audio_track_unmuted"),
        "audio-track-start-attempt" => Some("audio_track_start_attempt"),
        "audio-track-start-success" => Some("audio_track_start_success"),
        "audio-track-start-failed" => Some("audio_track_start_failed"),
        "fallback-default-start-attempt" => Some("fallback_default_start_attempt"),
        "fallback-default-start-success" => Some("fallback_default_start_success"),
        "fallback-default-start-failed" => Some("fallback_default_start_failed"),
        _ => None,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CoreStatusSnapshot {
    worker_connected: bool,
    desired_running: bool,
    recognition_running: bool,
    recognition_state: String,
    supervisor_state: String,
    websocket_ready: bool,
    browser_mode: String,
    start_mode: String,
    provider_name: String,
    session_id: String,
    client_segment_id: String,
    generation_id: u64,
    active_recognition: bool,
    active_media_stream: bool,
    pending_start: bool,
    recognition_continuous: bool,
    effective_continuous_mode: String,
    forced_final: bool,
    last_session_started_at_ms: Option<u64>,
    last_session_ended_at_ms: Option<u64>,
    browser_cycle_pending: bool,
    browser_cycle_count: u64,
    browser_minimum_reconnect_suppressed_count: u64,
    browser_forced_final_on_interruption_count: u64,
    mic_track_ready_state: String,
    mic_track_muted: bool,
    get_user_media_last_error: String,
    mic_stream_active: bool,
    visibility_state: String,
    last_error: String,
    error_type: String,
    degraded_reason: String,
    rearm_count: u64,
    restart_count: u64,
    watchdog_rearm_count: u64,
    stopping_since_ms: Option<u64>,
    overlap_mode_desired: bool,
    overlap_active: bool,
    overlap_active_slot: Option<u64>,
    overlap_buddy_slot: Option<u64>,
    overlap_prestarted: bool,
    overlap_active_listening: bool,
    overlap_buddy_listening: bool,
    overlap_prestart_timer_armed: bool,
}

#[derive(Debug, PartialEq)]
struct MaterialStatusSnapshot {
    core: CoreStatusSnapshot,
    last_result_index: Option<u64>,
    duplicate_partial_suppressed: u64,
    duplicate_final_suppressed: u64,
    late_forced_final_suppressed: u64,
    mic_rms: f64,
    get_user_media_count: u64,
    media_tracks_stopped_count: u64,
    media_track_leak_guard_count: u64,
    no_speech_count: u64,
    network_error_count: u64,
    stale_worker_events_ignored: u64,
}

fn should_log_status_snapshot(
    previous: &GatewayDiagnostics,
    current: &GatewayDiagnostics,
    reason: Option<&str>,
    mapped_event: Option<&str>,
    last_log: Instant,
    detail_interval_ms: u64,
) -> bool {
    if mapped_event.is_some() || reason == Some("degraded") {
        return false;
    }
    if matches!(
        reason,
        Some("socket-open")
            | Some("user-stop")
            | Some("terminal-error")
            | Some("microphone-permission-failed")
    ) {
        return true;
    }
    if core_status_snapshot(previous) != core_status_snapshot(current) {
        return true;
    }
    if material_status_snapshot(previous) == material_status_snapshot(current) {
        return false;
    }
    last_log.elapsed() >= Duration::from_millis(detail_interval_ms)
}

fn should_log_status_heartbeat(state: &GatewayDiagnostics, last_heartbeat: Instant) -> bool {
    if !(state.worker_connected || state.desired_running) {
        return false;
    }
    last_heartbeat.elapsed() >= Duration::from_millis(STATUS_HEARTBEAT_INTERVAL_MS)
}

fn core_status_snapshot(state: &GatewayDiagnostics) -> CoreStatusSnapshot {
    CoreStatusSnapshot {
        worker_connected: state.worker_connected,
        desired_running: state.desired_running,
        recognition_running: state.recognition_running,
        recognition_state: state.recognition_state.clone().unwrap_or_default(),
        supervisor_state: state.supervisor_state.clone().unwrap_or_default(),
        websocket_ready: state.websocket_ready,
        browser_mode: state.browser_mode.clone().unwrap_or_default(),
        start_mode: state.start_mode.clone().unwrap_or_default(),
        provider_name: state.provider_name.clone().unwrap_or_default(),
        session_id: state.session_id.clone().unwrap_or_default(),
        client_segment_id: state.client_segment_id.clone().unwrap_or_default(),
        generation_id: state.generation_id,
        active_recognition: state.active_recognition,
        active_media_stream: state.active_media_stream,
        pending_start: state.pending_start,
        recognition_continuous: state.recognition_continuous,
        effective_continuous_mode: state.effective_continuous_mode.clone().unwrap_or_default(),
        forced_final: state.forced_final,
        last_session_started_at_ms: state.last_session_started_at_ms,
        last_session_ended_at_ms: state.last_session_ended_at_ms,
        browser_cycle_pending: state.browser_cycle_pending,
        browser_cycle_count: state.browser_cycle_count,
        browser_minimum_reconnect_suppressed_count: state
            .browser_minimum_reconnect_suppressed_count,
        browser_forced_final_on_interruption_count: state
            .browser_forced_final_on_interruption_count,
        mic_track_ready_state: state.mic_track_ready_state.clone().unwrap_or_default(),
        mic_track_muted: state.mic_track_muted,
        get_user_media_last_error: state.get_user_media_last_error.clone().unwrap_or_default(),
        mic_stream_active: state.mic_stream_active,
        visibility_state: state.visibility_state.clone().unwrap_or_default(),
        last_error: state.last_error.clone().unwrap_or_default(),
        error_type: state.error_type.clone().unwrap_or_default(),
        degraded_reason: state.degraded_reason.clone().unwrap_or_default(),
        rearm_count: state.rearm_count,
        restart_count: state.restart_count,
        watchdog_rearm_count: state.watchdog_rearm_count,
        stopping_since_ms: state.stopping_since_ms,
        overlap_mode_desired: state.overlap_mode_desired,
        overlap_active: state.overlap_active,
        overlap_active_slot: state.overlap_active_slot,
        overlap_buddy_slot: state.overlap_buddy_slot,
        overlap_prestarted: state.overlap_prestarted,
        overlap_active_listening: state.overlap_active_listening,
        overlap_buddy_listening: state.overlap_buddy_listening,
        overlap_prestart_timer_armed: state.overlap_prestart_timer_armed,
    }
}

fn material_status_snapshot(state: &GatewayDiagnostics) -> MaterialStatusSnapshot {
    MaterialStatusSnapshot {
        core: core_status_snapshot(state),
        last_result_index: state.last_result_index,
        duplicate_partial_suppressed: state.duplicate_partial_suppressed,
        duplicate_final_suppressed: state.duplicate_final_suppressed,
        late_forced_final_suppressed: state.late_forced_final_suppressed,
        mic_rms: state.mic_rms.unwrap_or(0.0),
        get_user_media_count: state.get_user_media_count,
        media_tracks_stopped_count: state.media_tracks_stopped_count,
        media_track_leak_guard_count: state.media_track_leak_guard_count,
        no_speech_count: state.no_speech_count,
        network_error_count: state.network_error_count,
        stale_worker_events_ignored: state.stale_worker_events_ignored,
    }
}

fn overlap_status_fields(state: &GatewayDiagnostics) -> Value {
    json!({
        "overlap_mode_desired": state.overlap_mode_desired,
        "overlap_active": state.overlap_active,
        "overlap_active_slot": state.overlap_active_slot,
        "overlap_buddy_slot": state.overlap_buddy_slot,
        "overlap_prestarted": state.overlap_prestarted,
        "overlap_active_listening": state.overlap_active_listening,
        "overlap_buddy_listening": state.overlap_buddy_listening,
        "overlap_prestart_timer_armed": state.overlap_prestart_timer_armed,
    })
}

fn structured_status_log_summary(state: &GatewayDiagnostics, reason: Option<&str>) -> Value {
    let mut summary = json!({
        "reason": reason,
        "worker_connected": state.worker_connected,
        "browser_mode": state.browser_mode,
        "desired_running": state.desired_running,
        "recognition_running": state.recognition_running,
        "recognition_state": state.recognition_state,
        "supervisor_state": state.supervisor_state,
        "websocket_ready": state.websocket_ready,
        "provider_name": state.provider_name,
        "start_mode": state.start_mode,
        "generation_id": state.generation_id,
        "session_id": state.session_id,
        "client_segment_id": state.client_segment_id,
        "visibility_state": state.visibility_state,
        "rearm_count": state.rearm_count,
        "watchdog_rearm_count": state.watchdog_rearm_count,
        "error_type": state.error_type,
        "last_error": state.last_error,
        "degraded_reason": state.degraded_reason,
    });
    if let Some(obj) = summary.as_object_mut()
        && let Some(overlap) = overlap_status_fields(state).as_object()
    {
        obj.extend(overlap.clone());
    }
    summary
}

fn structured_mapped_event_log_summary(state: &GatewayDiagnostics) -> Value {
    let mut summary = json!({
        "recognition_state": state.recognition_state,
        "supervisor_state": state.supervisor_state,
        "generation_id": state.generation_id,
        "session_id": state.session_id,
        "client_segment_id": state.client_segment_id,
        "rearm_count": state.rearm_count,
        "error_type": state.error_type,
        "visibility_state": state.visibility_state,
    });
    if let Some(obj) = summary.as_object_mut()
        && let Some(overlap) = overlap_status_fields(state).as_object()
    {
        obj.extend(overlap.clone());
    }
    summary
}

fn heartbeat_payload(state: &GatewayDiagnostics, baseline: &CounterSnapshot) -> Value {
    let current = counter_snapshot(state);
    let counters_delta = [
        (
            "rearm_count",
            current.rearm_count.saturating_sub(baseline.rearm_count),
        ),
        (
            "restart_count",
            current.restart_count.saturating_sub(baseline.restart_count),
        ),
        (
            "watchdog_rearm_count",
            current
                .watchdog_rearm_count
                .saturating_sub(baseline.watchdog_rearm_count),
        ),
        (
            "no_speech_count",
            current
                .no_speech_count
                .saturating_sub(baseline.no_speech_count),
        ),
        (
            "network_error_count",
            current
                .network_error_count
                .saturating_sub(baseline.network_error_count),
        ),
        (
            "duplicate_partial_suppressed",
            current
                .duplicate_partial_suppressed
                .saturating_sub(baseline.duplicate_partial_suppressed),
        ),
        (
            "duplicate_final_suppressed",
            current
                .duplicate_final_suppressed
                .saturating_sub(baseline.duplicate_final_suppressed),
        ),
        (
            "late_forced_final_suppressed",
            current
                .late_forced_final_suppressed
                .saturating_sub(baseline.late_forced_final_suppressed),
        ),
        (
            "stale_worker_events_ignored",
            current
                .stale_worker_events_ignored
                .saturating_sub(baseline.stale_worker_events_ignored),
        ),
    ]
    .into_iter()
    .filter(|(_, delta)| *delta != 0)
    .map(|(key, delta)| (key.to_string(), json!(delta)))
    .collect::<serde_json::Map<_, _>>();

    json!({
        "state": state
            .recognition_state
            .clone()
            .or_else(|| state.supervisor_state.clone())
            .unwrap_or_else(|| "idle".into()),
        "generation_id": state.generation_id,
        "last_result_age_ms": last_result_age_ms(state),
        "overlap_active": state.overlap_active,
        "overlap_buddy_listening": state.overlap_buddy_listening,
        "overlap_prestarted": state.overlap_prestarted,
        "counters_delta": counters_delta,
    })
}

fn last_result_age_ms(state: &GatewayDiagnostics) -> Option<u64> {
    match (state.last_partial_age_ms, state.last_final_age_ms) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn should_log_mapped_event(event: &str, state: &GatewayDiagnostics) -> bool {
    if ROUTINE_RESTART_EVENTS.contains(&event) {
        return should_sample_routine_cycle(state);
    }
    if event == "browser_onerror"
        && state
            .error_type
            .as_deref()
            .is_some_and(|t| NOISY_ERROR_TYPES.contains(&t))
    {
        return should_sample_routine_cycle(state);
    }
    true
}

fn should_log_error_event(state: &GatewayDiagnostics) -> bool {
    if state
        .error_type
        .as_deref()
        .is_some_and(|t| NOISY_ERROR_TYPES.contains(&t))
    {
        return should_sample_routine_cycle(state);
    }
    true
}

fn should_sample_routine_cycle(state: &GatewayDiagnostics) -> bool {
    if state.degraded_reason.is_some() {
        return true;
    }
    let rearm_count = state.rearm_count;
    if rearm_count <= ROUTINE_LOG_VERBOSE_LIMIT {
        return true;
    }
    rearm_count.is_multiple_of(ROUTINE_LOG_SAMPLE_EVERY)
}
