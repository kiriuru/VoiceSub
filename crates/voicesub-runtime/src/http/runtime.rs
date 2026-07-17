use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use voicesub_browser::BrowserWorkerLauncher;
use voicesub_config::{
    ASR_MODE_LOCAL_PARAKEET, base_url_from_socket, overlay_url, read_full_logging_enabled,
    worker_url_for_payload,
};
use voicesub_logging::apply_logging_preferences;
use voicesub_translation::DispatcherCallbacks;

use super::asr_diagnostics::assemble_browser_asr_diagnostics;
use super::local_asr::local_asr_module_json;
use super::partial_emit::partial_emit_settings_from_config;
use super::state::HttpState;

#[derive(Debug)]
pub(crate) struct OrchestratorInner {
    running: bool,
    phase: &'static str,
    started_at_utc: Option<String>,
    last_error: Option<String>,
    worker_pid: Option<u32>,
    status_message: Option<String>,
}

impl Default for OrchestratorInner {
    fn default() -> Self {
        Self {
            running: false,
            phase: "idle",
            started_at_utc: None,
            last_error: None,
            worker_pid: None,
            status_message: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct RuntimeOrchestrator {
    pub(crate) inner: Arc<Mutex<OrchestratorInner>>,
}

impl RuntimeOrchestrator {
    pub async fn start(&self, state: &HttpState, config_payload: Option<Value>) -> Value {
        state.pipeline_log.start_begin();
        state.runtime_broadcaster.broadcast_preflight(true).await;
        {
            let mut inner = self.inner.lock().await;
            inner.phase = "starting";
            inner.last_error = None;
        }

        if let Some(payload) = config_payload {
            let snapshot_payload = {
                let mut store = state.config.write().await;
                if let Err(err) = store.apply_save_payload(&payload) {
                    let mut inner = self.inner.lock().await;
                    inner.phase = "error";
                    inner.last_error = Some(err.to_string());
                    inner.running = false;
                    let response = runtime_action_response("start", &inner, state).await;
                    state.runtime_broadcaster.broadcast_preflight(false).await;
                    return response;
                }
                store.payload().clone()
            };
            if let Ok(mut snapshot) = state.config_snapshot.write() {
                *snapshot = snapshot_payload.clone();
            }
            apply_logging_preferences(
                &state.paths.logs_dir,
                read_full_logging_enabled(&snapshot_payload),
            );
            state.translation.lock().await.apply_live_settings();
            state.subtitle.republish_latest().await;
        }

        let config_payload = state.config.read().await;
        let payload = config_payload.payload().clone();
        let asr_mode = asr_mode_from_payload(&payload);
        let use_local_asr = asr_mode == ASR_MODE_LOCAL_PARAKEET;
        drop(config_payload);

        if use_local_asr {
            let module_status = state.local_asr.status();
            if !module_status.ready {
                let mut inner = self.inner.lock().await;
                inner.phase = "error";
                inner.last_error = Some(format!(
                    "Local ASR module is not ready: {}",
                    module_status.message
                ));
                inner.running = false;
                inner.status_message = None;
                drop(inner);
                state.runtime_broadcaster.broadcast_preflight(false).await;
                let inner = self.inner.lock().await;
                let response = runtime_action_response("start", &inner, state).await;
                broadcast_runtime_update(state, &inner, true).await;
                return response;
            }
        }

        let previous = {
            let inner = self.inner.lock().await;
            (inner.phase.to_string(), inner.running)
        };

        let started_at = utc_now_stamp();
        {
            let mut partial_emit = state.partial_emit.lock().await;
            partial_emit.reset();
        }
        state
            .runtime_running
            .store(true, std::sync::atomic::Ordering::Relaxed);
        state.browser_speech.start().await;
        state
            .translation
            .lock()
            .await
            .start(structured_log_callbacks(state))
            .await;
        state.obs_captions.start().await;
        state.obs_captions.apply_live_settings().await;
        state.subtitle.reset().await;
        state.runtime_metrics.reset();

        if use_local_asr {
            {
                let module_status = state.local_asr.status();
                let mut inner = self.inner.lock().await;
                inner.status_message = Some(local_asr_startup_message(&module_status));
            }
            {
                let inner = self.inner.lock().await;
                broadcast_runtime_update(state, &inner, true).await;
            }

            match state.local_asr_speech.start().await {
                Ok(()) => {
                    let mut inner = self.inner.lock().await;
                    inner.worker_pid = None;
                    inner.running = true;
                    inner.phase = "listening";
                    inner.started_at_utc = Some(started_at);
                    inner.status_message = None;
                    state.pipeline_log.state_changed(
                        &previous.0,
                        inner.phase,
                        previous.1,
                        inner.running,
                        None,
                    );
                    state.pipeline_log.start_complete(inner.phase, None);
                    drop(inner);
                    let inner = self.inner.lock().await;
                    let response = runtime_action_response("start", &inner, state).await;
                    broadcast_runtime_update(state, &inner, true).await;
                    return response;
                }
                Err(err) => {
                    let _ = state.local_asr_speech.stop().await;
                    let mut inner = self.inner.lock().await;
                    inner.phase = "error";
                    inner.last_error = Some(err.clone());
                    inner.running = false;
                    inner.status_message = None;
                    drop(inner);
                    state
                        .runtime_running
                        .store(false, std::sync::atomic::Ordering::Relaxed);
                    state.browser_speech.stop().await;
                    state.obs_captions.stop().await;
                    state.translation.lock().await.stop().await;
                    {
                        let mut partial_emit = state.partial_emit.lock().await;
                        partial_emit.reset();
                    }
                    let inner = self.inner.lock().await;
                    let response = runtime_action_response("start", &inner, state).await;
                    broadcast_runtime_update(state, &inner, true).await;
                    state.runtime_broadcaster.broadcast_preflight(false).await;
                    return response;
                }
            }
        }

        let base = resolve_base_url(state).await;
        let config_payload = state.config.read().await;
        let payload = config_payload.payload().clone();
        let worker_target = worker_url_for_payload(&base, &payload);
        let chrome_launch = voicesub_browser::chrome_launch_from_config(&payload);
        drop(config_payload);
        let launcher = BrowserWorkerLauncher::new(&state.paths.user_data_dir);

        let previous_worker_pid = {
            let inner = self.inner.lock().await;
            inner.worker_pid
        };
        if let Some(pid) = previous_worker_pid {
            if BrowserWorkerLauncher::terminate_worker(pid) {
                tracing::info!(pid, "terminated previous browser worker before relaunch");
            } else {
                tracing::warn!(
                    pid,
                    "failed to terminate previous browser worker before relaunch"
                );
            }
        }

        let launch_result = launcher.launch_worker(&worker_target, &chrome_launch);
        let mut inner = self.inner.lock().await;
        match launch_result {
            Ok(result) => {
                inner.worker_pid = if result.pid == 0 {
                    None
                } else {
                    Some(result.pid)
                };
                // Persist the live worker PID so a crash before graceful stop can reap the
                // leftover high-priority Chrome on next startup (review §8).
                if let Some(pid) = inner.worker_pid {
                    voicesub_browser::record_worker_pid(&state.paths.user_data_dir, pid);
                }
                inner.running = true;
                inner.phase = "listening";
                inner.started_at_utc = Some(started_at);
                state.pipeline_log.state_changed(
                    &previous.0,
                    inner.phase,
                    previous.1,
                    inner.running,
                    None,
                );
                state
                    .pipeline_log
                    .start_complete(inner.phase, inner.worker_pid);
            }
            Err(err) => {
                inner.phase = "error";
                inner.last_error = Some(err.to_string());
                inner.running = false;
                drop(inner);
                state.obs_captions.stop().await;
                state.translation.lock().await.stop().await;
                let inner = self.inner.lock().await;
                let response = runtime_action_response("start", &inner, state).await;
                broadcast_runtime_update(state, &inner, true).await;
                state.runtime_broadcaster.broadcast_preflight(false).await;
                return response;
            }
        }
        drop(inner);

        let inner = self.inner.lock().await;
        let response = runtime_action_response("start", &inner, state).await;
        broadcast_runtime_update(state, &inner, true).await;
        response
    }

    pub async fn stop(&self, state: &HttpState) -> Value {
        state.pipeline_log.stop_begin();
        state.runtime_broadcaster.broadcast_preflight(false).await;
        let worker_pid = {
            let inner = self.inner.lock().await;
            inner.worker_pid
        };

        if let Err(err) = state.local_asr_speech.stop().await {
            tracing::warn!(error = %err, "local ASR runtime capture stop failed");
        }

        let _ = state
            .asr_worker
            .send_control("stop", Some("runtime_stop"))
            .await;

        if let Some(pid) = worker_pid
            && BrowserWorkerLauncher::terminate_worker(pid)
        {
            tracing::info!(pid, "browser worker process terminated");
        }
        // Graceful stop: drop the persisted PID so startup reaping does not target a PID
        // that may later be reused by another process (review §8).
        voicesub_browser::clear_worker_pid(&state.paths.user_data_dir);

        state.subtitle.reset().await;
        state.flush_overlay_presentations_to_clients().await;
        state.translation.lock().await.stop().await;
        state.obs_captions.stop().await;
        state.runtime_broadcaster.reset_broadcast_state();

        log_runtime_stop(state);
        state
            .runtime_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        state.browser_speech.stop().await;
        {
            let mut partial_emit = state.partial_emit.lock().await;
            partial_emit.reset();
        }

        state.runtime_metrics.reset();

        let mut inner = self.inner.lock().await;
        let previous = (inner.phase.to_string(), inner.running);
        inner.running = false;
        inner.phase = "idle";
        inner.worker_pid = None;
        inner.started_at_utc = None;
        inner.status_message = None;
        state.pipeline_log.state_changed(
            &previous.0,
            inner.phase,
            previous.1,
            inner.running,
            inner.last_error.as_deref(),
        );
        state.pipeline_log.stop_complete();
        let response = runtime_action_response("stop", &inner, state).await;
        broadcast_runtime_update(state, &inner, true).await;
        response
    }

    pub async fn status(&self, state: &HttpState) -> Value {
        let inner = self.inner.lock().await;
        build_runtime_status(&inner, state).await
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct RuntimeStartRequest {
    #[serde(default)]
    pub config_payload: Option<Value>,
}

pub async fn runtime_start(
    State(state): State<Arc<HttpState>>,
    Json(body): Json<RuntimeStartRequest>,
) -> Response {
    let payload = body.config_payload;
    let result = state.orchestrator.start(state.as_ref(), payload).await;
    Json(result).into_response()
}

pub async fn runtime_stop(State(state): State<Arc<HttpState>>) -> Response {
    let result = state.orchestrator.stop(state.as_ref()).await;
    Json(result).into_response()
}

pub async fn runtime_status(State(state): State<Arc<HttpState>>) -> Response {
    let result = state.orchestrator.status(state.as_ref()).await;
    Json(result).into_response()
}

pub async fn obs_url(State(state): State<Arc<HttpState>>) -> Response {
    let base = resolve_base_url(&state).await;
    Json(json!({
        "overlay_url": overlay_url(&base)
    }))
    .into_response()
}

pub(crate) async fn resolve_base_url(state: &HttpState) -> String {
    if let Some(addr) = *state.bind_addr.read().await {
        return base_url_from_socket(addr);
    }
    state.app_config.http.base_url()
}

async fn runtime_action_response(
    action: &str,
    inner: &OrchestratorInner,
    state: &HttpState,
) -> Value {
    json!({
        "ok": inner.last_error.is_none(),
        "action": action,
        "runtime": build_runtime_status(inner, state).await
    })
}

async fn build_runtime_status(inner: &OrchestratorInner, state: &HttpState) -> Value {
    let browser_diag = state.asr_worker.service().diagnostics().await;
    let ws_diag = state.events.diagnostics();
    let translation_diag = state.translation.lock().await.diagnostics_snapshot();
    let base = resolve_base_url(state).await;
    let store = state.config.read().await;
    let payload = store.payload();
    let asr_mode = asr_mode_from_payload(payload);
    let use_local_asr = asr_mode == ASR_MODE_LOCAL_PARAKEET;
    let obs_diag = state.obs_captions.diagnostics().await;
    let obs_captions = json!({
        "enabled": obs_diag.get("enabled").cloned().unwrap_or(json!(false)),
        "active": obs_diag.get("active").cloned().unwrap_or(json!(false)),
        "connected": obs_diag.get("connected").cloned().unwrap_or(json!(false)),
        "connection_state": obs_diag.get("connection_state").cloned().unwrap_or(json!("disabled")),
        "output_mode": obs_diag.get("output_mode").cloned().unwrap_or(json!("disabled")),
        "diagnostics": obs_diag.clone(),
    });
    let subtitle_router_counters = state.subtitle.diagnostic_counters();
    let browser_lang = payload
        .get("asr")
        .and_then(|v| v.get("browser"))
        .and_then(|v| v.get("recognition_language"))
        .and_then(|v| v.as_str())
        .unwrap_or("en-US");
    let partial_emit = partial_emit_settings_from_config(payload);
    let asr_diagnostics = if use_local_asr {
        state.local_asr.diagnostics()
    } else {
        assemble_browser_asr_diagnostics(
            asr_mode,
            browser_lang,
            &browser_diag,
            &partial_emit,
            inner.running,
        )
    };

    let mut metrics = state.runtime_metrics.snapshot(
        &ws_diag,
        browser_diag.browser_stale_events_ignored,
        &translation_diag,
    );
    if let Some(obj) = metrics.as_object_mut() {
        if let Some(bus) = state.ws_publisher.event_bus() {
            let bus_diag = bus.diagnostics();
            obj.insert(
                "event_bus_subscribers".into(),
                json!(bus_diag.subscriber_count),
            );
            obj.insert("event_bus_revision".into(), json!(bus_diag.revision));
            obj.insert(
                "event_bus_publish_count".into(),
                json!(bus_diag.publish_count),
            );
            obj.insert(
                "event_bus_channel_capacity".into(),
                json!(bus_diag.channel_capacity),
            );
        }
        obj.insert("background_tasks".into(), state.background_tasks.snapshot());
    }

    let local_module = local_asr_module_json(&state.local_asr.status());

    json!({
        "running": inner.running,
        "starting": inner.phase == "starting",
        "stopping": false,
        "degraded_mode": if use_local_asr {
            false
        } else {
            browser_diag.degraded_reason.is_some()
        },
        "fallback_reason": null,
        "phase": inner.phase,
        "status": inner.phase,
        "is_running": inner.running,
        "started_at_utc": inner.started_at_utc,
        "last_error": inner.last_error,
        "status_message": inner.status_message,
        "active_config_source": store.document().loaded_from(),
        "asr": {
            "active_mode": asr_mode,
            "local_module": local_module,
            "diagnostics": {
                "browser_worker": browser_diag
            }
        },
        "asr_diagnostics": asr_diagnostics,
        "translation_diagnostics": translation_diag,
        "obs_captions": obs_captions,
        "obs_caption_diagnostics": obs_diag,
        "subtitle_router_counters": subtitle_router_counters,
        "overlay": {
            "overlay_url": overlay_url(&base)
        },
        "metrics": metrics
    })
}

async fn broadcast_runtime_update(state: &HttpState, inner: &OrchestratorInner, force: bool) {
    let runtime = build_runtime_status(inner, state).await;
    state
        .runtime_broadcaster
        .broadcast_runtime(runtime, force)
        .await;
}

fn local_asr_startup_message(status: &voicesub_asr_local::LocalAsrModuleStatus) -> String {
    let model = voicesub_asr_local::model_display_label(
        &status.active_model_family,
        &status.active_model_variant,
    );
    format!(
        "Loading {} ({})…",
        model,
        status.execution_provider.to_ascii_uppercase()
    )
}

fn asr_mode_from_payload(payload: &Value) -> &str {
    payload
        .get("asr")
        .and_then(|v| v.get("mode"))
        .and_then(|v| v.as_str())
        .unwrap_or(voicesub_config::ASR_MODE_BROWSER)
}

fn log_runtime_stop(state: &HttpState) {
    let stopped_at = utc_now_stamp();
    let mut details = BTreeMap::new();
    details.insert("stopped_at".into(), Value::String(stopped_at));
    state.structured_runtime_logger.log(
        "dashboard",
        "runtime_stop_session_marker",
        Some("runtime"),
        Some(details),
    );
}

fn structured_log_callbacks(state: &HttpState) -> DispatcherCallbacks {
    let structured_logger = state.structured_runtime_logger.clone();
    let runtime_metrics = state.runtime_metrics.clone();
    DispatcherCallbacks {
        structured_log: Some(Arc::new(move |channel, message, details| {
            let mut map = BTreeMap::new();
            if let Some(obj) = details.as_object() {
                for (key, value) in obj {
                    map.insert(key.clone(), value.clone());
                }
            }
            structured_logger.log(channel, message, Some("translation_dispatcher"), Some(map));
        })),
        metrics_callback: Some(Arc::new(move |snapshot| {
            runtime_metrics.record_translation_metrics(snapshot);
        })),
    }
}

fn utc_now_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}
