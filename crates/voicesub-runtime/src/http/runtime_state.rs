use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use std::sync::Arc;

use serde_json::Value;
use voicesub_ws::WsEventPublisher;

use super::metrics::RuntimeMetricsCollector;
use crate::trace::RuntimePipelineLog;

/// Material status fields used for runtime_update coalescing (SST `runtime_material_status_snapshot`).
fn runtime_material_status_snapshot(payload: &Value) -> Vec<Value> {
    let asr = payload.get("asr").and_then(|v| v.as_object());
    let asr_diagnostics = payload
        .get("asr_diagnostics")
        .and_then(|v| v.as_object());
    let browser_worker = asr_diagnostics
        .and_then(|d| d.get("browser_worker"))
        .and_then(|v| v.as_object());

    vec![
        payload.get("is_running").cloned().unwrap_or(Value::Null),
        payload
            .get("status")
            .or_else(|| payload.get("phase"))
            .cloned()
            .unwrap_or(Value::Null),
        payload.get("last_error").cloned().unwrap_or(Value::Null),
        payload.get("status_message").cloned().unwrap_or(Value::Null),
        asr.and_then(|a| a.get("active_mode")).cloned().unwrap_or(Value::Null),
        asr.and_then(|a| a.get("effective_provider"))
            .cloned()
            .unwrap_or(Value::Null),
        asr.and_then(|a| a.get("provider"))
            .cloned()
            .unwrap_or(Value::Null),
        asr.and_then(|a| a.get("provider_phase"))
            .cloned()
            .unwrap_or(Value::Null),
        asr.and_then(|a| a.get("provider_message"))
            .cloned()
            .unwrap_or(Value::Null),
        asr.and_then(|a| a.get("provider_error_kind"))
            .cloned()
            .unwrap_or(Value::Null),
        browser_worker
            .and_then(|b| b.get("worker_connected"))
            .cloned()
            .unwrap_or(Value::Null),
        browser_worker
            .and_then(|b| b.get("recognition_state"))
            .cloned()
            .unwrap_or(Value::Null),
        browser_worker
            .and_then(|b| b.get("supervisor_state"))
            .cloned()
            .unwrap_or(Value::Null),
        browser_worker
            .and_then(|b| b.get("degraded_reason"))
            .cloned()
            .unwrap_or(Value::Null),
        browser_worker
            .and_then(|b| b.get("last_error"))
            .cloned()
            .unwrap_or(Value::Null),
        browser_worker
            .and_then(|b| b.get("generation_id"))
            .cloned()
            .unwrap_or(Value::Null),
    ]
}

/// Port of SST `RuntimeStateController` broadcast coalescing + enrichment wiring.
pub struct RuntimeStatusBroadcaster {
    publisher: WsEventPublisher,
    pipeline_log: RuntimePipelineLog,
    metrics: Arc<RuntimeMetricsCollector>,
    last_runtime_signature: Mutex<Option<Vec<Value>>>,
    last_auxiliary_signatures: Mutex<HashMap<String, Vec<Value>>>,
    last_broadcast_at: Mutex<Option<Instant>>,
    pub(crate) heartbeat_interval_ms: u64,
}

impl RuntimeStatusBroadcaster {
    pub fn new(
        publisher: WsEventPublisher,
        heartbeat_interval_ms: u64,
        pipeline_log: RuntimePipelineLog,
        metrics: Arc<RuntimeMetricsCollector>,
    ) -> Self {
        Self {
            publisher,
            pipeline_log,
            metrics,
            last_runtime_signature: Mutex::new(None),
            last_auxiliary_signatures: Mutex::new(HashMap::new()),
            last_broadcast_at: Mutex::new(None),
            heartbeat_interval_ms: heartbeat_interval_ms.max(1),
        }
    }

    pub fn reset_broadcast_state(&self) {
        self.publisher.reset_broadcast_state();
        if let Ok(mut signature) = self.last_runtime_signature.lock() {
            *signature = None;
        }
        if let Ok(mut signatures) = self.last_auxiliary_signatures.lock() {
            signatures.clear();
        }
        if let Ok(mut last) = self.last_broadcast_at.lock() {
            *last = None;
        }
    }

    pub fn publisher(&self) -> WsEventPublisher {
        self.publisher.clone()
    }

    pub async fn broadcast_runtime(&self, runtime: Value, force: bool) {
        let signature = runtime_material_status_snapshot(&runtime);
        let now = Instant::now();
        let last_signature = self
            .last_runtime_signature
            .lock()
            .ok()
            .and_then(|guard| guard.clone());
        let important_change = last_signature.as_ref() != Some(&signature);
        let mut should_send = force;
        if !force {
            let last_at = self
                .last_broadcast_at
                .lock()
                .ok()
                .and_then(|guard| *guard);
            let elapsed_ms = last_at
                .map(|previous| now.duration_since(previous).as_millis() as u64)
                .unwrap_or(self.heartbeat_interval_ms);
            should_send = important_change || elapsed_ms >= self.heartbeat_interval_ms;
        }

        if !should_send {
            self.pipeline_log.runtime_status_duplicate_suppressed();
            self.metrics.record_runtime_status_duplicate_suppressed();
            return;
        }

        if important_change || force {
            self.metrics.record_runtime_status_broadcast();
            self.pipeline_log
                .runtime_status_broadcast(important_change || force, false);
        } else {
            self.pipeline_log.runtime_status_heartbeat_sent();
            self.metrics.record_runtime_status_heartbeat_sent();
            self.pipeline_log.runtime_status_broadcast(false, true);
        }

        if let Ok(mut guard) = self.last_runtime_signature.lock() {
            *guard = Some(signature);
        }
        if let Ok(mut guard) = self.last_broadcast_at.lock() {
            *guard = Some(now);
        }

        self.publisher
            .broadcast_channel("runtime_update", "runtime_status", runtime.clone())
            .await;
        self.broadcast_auxiliary(&runtime, force).await;
    }

    pub async fn broadcast_preflight(&self, running: bool) {
        self.publisher
            .broadcast_channel(
                "preflight_update",
                "preflight_update",
                serde_json::json!({ "running": running }),
            )
            .await;
    }

    async fn broadcast_auxiliary(&self, runtime: &Value, force: bool) {
        if let Some(diagnostics) = runtime.get("asr_diagnostics") {
            self.maybe_broadcast_diagnostics(diagnostics.clone(), force)
                .await;
        }
        let model = super::asr_diagnostics::assemble_model_status_from_runtime(runtime);
        self.maybe_broadcast_model_status(model, force).await;
    }

    async fn maybe_broadcast_diagnostics(&self, payload: Value, force: bool) {
        let signature = diagnostics_material_snapshot(&payload);
        if !force && !self.should_broadcast_auxiliary("diagnostics", &signature) {
            return;
        }
        self.publisher
            .broadcast_channel("diagnostics_update", "diagnostics_update", payload)
            .await;
    }

    async fn maybe_broadcast_model_status(&self, payload: Value, force: bool) {
        let signature = model_material_snapshot(&payload);
        if !force && !self.should_broadcast_auxiliary("model", &signature) {
            return;
        }
        self.publisher
            .broadcast_channel("model_status_update", "model_status_update", payload)
            .await;
    }

    fn should_broadcast_auxiliary(&self, kind: &str, signature: &[Value]) -> bool {
        let Ok(mut guard) = self.last_auxiliary_signatures.lock() else {
            return true;
        };
        if guard.get(kind) == Some(&signature.to_vec()) {
            return false;
        }
        guard.insert(kind.to_string(), signature.to_vec());
        true
    }
}

fn diagnostics_material_snapshot(payload: &Value) -> Vec<Value> {
    vec![
        payload.get("provider").cloned().unwrap_or(Value::Null),
        payload.get("degraded_mode").cloned().unwrap_or(Value::Null),
        payload.get("provider_phase").cloned().unwrap_or(Value::Null),
        payload.get("provider_message").cloned().unwrap_or(Value::Null),
        payload
            .get("browser_worker")
            .and_then(|v| v.get("worker_connected"))
            .cloned()
            .unwrap_or(Value::Null),
        payload
            .get("browser_worker")
            .and_then(|v| v.get("recognition_state"))
            .cloned()
            .unwrap_or(Value::Null),
        payload
            .get("partial_emit_mode")
            .cloned()
            .unwrap_or(Value::Null),
    ]
}

fn model_material_snapshot(payload: &Value) -> Vec<Value> {
    vec![
        payload.get("status").cloned().unwrap_or(Value::Null),
        payload.get("loaded").cloned().unwrap_or(Value::Null),
        payload.get("degraded").cloned().unwrap_or(Value::Null),
        payload.get("message").cloned().unwrap_or(Value::Null),
    ]
}

pub fn spawn_runtime_heartbeat(
    broadcaster: std::sync::Arc<RuntimeStatusBroadcaster>,
    state: std::sync::Arc<super::state::HttpState>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(
            broadcaster.heartbeat_interval_ms,
        ));
        loop {
            interval.tick().await;
            let runtime = state.orchestrator.status(state.as_ref()).await;
            broadcaster.broadcast_runtime(runtime, false).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn runtime_material_status_snapshot_changes_when_worker_connects() {
        let disconnected = json!({
            "is_running": true,
            "status": "listening",
            "asr": { "active_mode": "browser_google" },
            "asr_diagnostics": { "browser_worker": { "worker_connected": false } }
        });
        let connected = json!({
            "is_running": true,
            "status": "listening",
            "asr": { "active_mode": "browser_google" },
            "asr_diagnostics": { "browser_worker": { "worker_connected": true } }
        });
        assert_ne!(
            runtime_material_status_snapshot(&disconnected),
            runtime_material_status_snapshot(&connected)
        );
        assert_eq!(
            runtime_material_status_snapshot(&connected),
            runtime_material_status_snapshot(&connected)
        );
    }
}
