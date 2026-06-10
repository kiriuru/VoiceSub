use serde_json::{json, Value};
use voicesub_browser::BrowserAsrDiagnostics;

use super::partial_emit::PartialEmitSettings;

/// Browser-only ASR diagnostics snapshot (SST `assemble_browser_asr_diagnostics`).
pub fn assemble_browser_asr_diagnostics(
    asr_mode: &str,
    browser_lang: &str,
    browser_worker: &BrowserAsrDiagnostics,
    partial_emit: &PartialEmitSettings,
    is_runtime_running: bool,
) -> Value {
    let worker_connected = browser_worker.worker_connected;
    let worker_message = if worker_connected {
        "Browser speech worker is connected."
    } else {
        "Open the browser speech window and start recognition there."
    };
    let provider_phase = if browser_worker.recognition_state.is_empty() {
        browser_worker.supervisor_state.clone()
    } else {
        browser_worker.recognition_state.clone()
    };

    let mut body = serde_json::Map::new();
    body.insert("mode".into(), json!(asr_mode));
    body.insert("provider_preference".into(), json!(asr_mode));
    body.insert("effective_provider".into(), json!(asr_mode));
    body.insert("provider".into(), json!(asr_mode));
    body.insert("provider_label".into(), json!("Browser Google Speech"));
    body.insert("provider_kind".into(), json!("browser_worker"));
    body.insert("provider_mode_kind".into(), json!("browser_speech"));
    body.insert("uses_browser_worker".into(), json!(true));
    body.insert("uses_backend_audio_capture".into(), json!(false));
    body.insert("true_streaming".into(), json!(true));
    body.insert("requested_provider".into(), json!(asr_mode));
    body.insert("requested_device_policy".into(), json!("browser_window"));
    body.insert("requested_device".into(), json!("browser_window"));
    body.insert("cuda_available".into(), json!(false));
    body.insert("supports_gpu".into(), json!(false));
    body.insert("supports_partials".into(), json!(true));
    body.insert("supports_streaming".into(), json!(true));
    body.insert("gpu_requested".into(), json!(false));
    body.insert("gpu_available".into(), json!(false));
    body.insert("torch_built_with_cuda".into(), json!(false));
    body.insert("torch_cuda_is_available".into(), json!(false));
    body.insert("torch_device_count".into(), json!(0));
    body.insert(
        "degraded_mode".into(),
        json!(browser_worker.degraded_reason.is_some()),
    );
    body.insert("selected_device".into(), json!("browser"));
    body.insert(
        "selected_execution_provider".into(),
        json!("webkitSpeechRecognition"),
    );
    body.insert("partials_supported".into(), json!(true));
    body.insert("recognition_noise_reduction_enabled".into(), json!(false));
    body.insert("rnnoise_strength".into(), json!(0));
    body.insert("rnnoise_available".into(), json!(false));
    body.insert("rnnoise_active".into(), json!(false));
    body.insert(
        "rnnoise_message".into(),
        json!("RNNoise is not used in browser speech mode."),
    );
    body.insert("provider_phase".into(), json!(provider_phase));
    body.insert("provider_message".into(), json!(worker_message));
    if let Some(last_error) = browser_worker.last_error.clone() {
        body.insert("provider_last_error".into(), json!(last_error));
    }
    body.insert(
        "message".into(),
        json!(format!("{worker_message} Recognition language: {browser_lang}.")),
    );
    body.insert("runtime_initialized".into(), json!(is_runtime_running));
    body.insert(
        "browser_worker".into(),
        serde_json::to_value(browser_worker).unwrap_or(Value::Null),
    );
    body.insert(
        "partial_emit_mode".into(),
        json!(partial_emit.partial_emit_mode),
    );
    body.insert(
        "partial_min_new_words".into(),
        json!(partial_emit.partial_min_new_words),
    );
    body.insert(
        "partial_min_delta_chars".into(),
        json!(partial_emit.partial_min_delta_chars),
    );
    body.insert(
        "partial_coalescing_ms".into(),
        json!(partial_emit.partial_coalescing_ms),
    );
    body.insert("streaming_decode".into(), json!(true));
    Value::Object(body)
}

pub fn assemble_model_status_from_runtime(runtime: &Value) -> Value {
    let asr_mode = runtime
        .get("asr")
        .and_then(|v| v.get("active_mode"))
        .and_then(|v| v.as_str())
        .unwrap_or("browser_google");
    let is_runtime_running = runtime
        .get("is_running")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        || runtime
            .get("running")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    if let Some(worker_value) = runtime
        .get("asr_diagnostics")
        .and_then(|v| v.get("browser_worker"))
    {
        let worker_connected = worker_value
            .get("worker_connected")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let degraded = worker_value
            .get("degraded_reason")
            .map(|v| !v.is_null())
            .unwrap_or(false);
        let details = json!({
            "worker_connected": worker_connected,
            "recognition_state": worker_value.get("recognition_state").cloned().unwrap_or(Value::Null),
            "generation_id": worker_value.get("generation_id").cloned().unwrap_or(Value::Null),
        });
        return model_status_body(
            asr_mode,
            is_runtime_running && worker_connected,
            is_runtime_running,
            degraded,
            details,
        );
    }
    let degraded = runtime
        .get("asr_diagnostics")
        .and_then(|v| v.get("degraded_mode"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    model_status_body(asr_mode, false, is_runtime_running, degraded, Value::Null)
}

fn model_status_body(
    asr_mode: &str,
    loaded: bool,
    is_runtime_running: bool,
    degraded: bool,
    details: Value,
) -> Value {
    json!({
        "status": if loaded { "ready" } else if is_runtime_running { "waiting" } else { "idle" },
        "message": if loaded {
            "Browser speech worker is connected."
        } else if is_runtime_running {
            "Waiting for browser speech worker connection."
        } else {
            "Browser speech worker mode (no local ASR model)."
        },
        "provider": asr_mode,
        "loaded": loaded,
        "available": true,
        "degraded": degraded,
        "details": details,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use voicesub_browser::BrowserAsrDiagnostics;

    #[test]
    fn browser_diagnostics_include_partial_emit_fields() {
        let worker = BrowserAsrDiagnostics {
            worker_connected: true,
            recognition_state: "running".into(),
            ..Default::default()
        };
        let partial = PartialEmitSettings {
            partial_emit_mode: "word_growth".into(),
            partial_min_new_words: 2,
            partial_min_delta_chars: 1,
            partial_coalescing_ms: 100,
        };
        let body = assemble_browser_asr_diagnostics(
            "browser_google",
            "en-US",
            &worker,
            &partial,
            true,
        );
        assert_eq!(body["true_streaming"], true);
        assert_eq!(body["partial_emit_mode"], "word_growth");
        assert_eq!(body["partial_min_new_words"], 2);
        assert_eq!(body["uses_browser_worker"], true);
    }

    #[test]
    fn model_status_ready_when_worker_connected() {
        let runtime = json!({
            "is_running": true,
            "asr": { "active_mode": "browser_google" },
            "asr_diagnostics": {
                "browser_worker": {
                    "worker_connected": true,
                    "recognition_state": "running",
                    "generation_id": 1
                }
            }
        });
        let body = assemble_model_status_from_runtime(&runtime);
        assert_eq!(body["status"], "ready");
        assert_eq!(body["loaded"], true);
    }
}
