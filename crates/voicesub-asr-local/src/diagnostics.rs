//! SST `asr_diagnostics_assembler` — Local Parakeet diagnostics snapshot.

use serde_json::{Value, json};
use voicesub_partial_emit::PartialEmitSettings;

use crate::config::LocalAsrConfig;
use crate::deps::LocalAsrEnvCheck;
use crate::emit_telemetry::EmitTelemetrySnapshot;
use crate::inference::InferenceSnapshot;
use crate::realtime_settings::ResolvedRealtimeSettings;
use crate::status::LocalAsrModulePhase;

#[derive(Debug, Clone)]
pub struct LocalAsrDiagnosticsInput<'a> {
    pub config: &'a LocalAsrConfig,
    pub env: &'a LocalAsrEnvCheck,
    pub inference: &'a InferenceSnapshot,
    pub phase: LocalAsrModulePhase,
    pub is_runtime_running: bool,
    pub decode_count: u64,
    pub finalized_segments: u64,
    pub emit_telemetry: Option<&'a EmitTelemetrySnapshot>,
    /// Last adaptive partial decode interval applied by the live/test pipeline (0 if unused).
    pub last_paced_decode_interval_ms: u32,
    /// Last decode wall ms reported by the decode worker (0 if unused).
    pub last_decode_wall_ms: u64,
}

pub fn assemble_local_asr_diagnostics(input: LocalAsrDiagnosticsInput<'_>) -> Value {
    let realtime = ResolvedRealtimeSettings::from_config(input.config);
    let partial = &realtime.partial_emit;
    let provider = input.config.inference.execution_provider.clone();
    let active_ep = if input.inference.model_loaded {
        input.inference.active_execution_provider.clone()
    } else {
        provider.clone()
    };
    let degraded = input.inference.last_error.is_some()
        || (provider == "cuda" && input.inference.probe_cuda_ok == Some(false));
    let tel = input.emit_telemetry;
    let partial_emits = tel.map(|t| t.partial_emits).unwrap_or(input.decode_count);
    let final_emits = tel
        .map(|t| t.final_emits)
        .unwrap_or(input.finalized_segments);

    json!({
        "mode": "local_parakeet",
        "provider": "local_parakeet",
        "provider_label": "Local Parakeet TDT",
        "provider_kind": "local_onnx",
        "uses_browser_worker": false,
        "uses_backend_audio_capture": true,
        "true_streaming": realtime.streaming_decode,
        "supports_partials": true,
        "partials_supported": true,
        "degraded_mode": degraded,
        "selected_device": input.config.microphone.device_id,
        "selected_execution_provider": active_ep,
        "provider_phase": format!("{:?}", input.phase).to_ascii_lowercase(),
        "provider_message": diagnostics_message(&input),
        "provider_last_error": input.inference.last_error,
        "runtime_initialized": input.is_runtime_running,
        "latency_preset": realtime.latency_preset,
        "streaming_decode": realtime.streaming_decode,
        "partial_emit_mode": partial.partial_emit_mode,
        "partial_min_new_words": partial.partial_min_new_words,
        "partial_min_delta_chars": partial.partial_min_delta_chars,
        "partial_coalescing_ms": partial.partial_coalescing_ms,
        "decode_interval_ms": realtime.decode_interval_ms,
        "window_ms": realtime.window_ms,
        "last_paced_decode_interval_ms": input.last_paced_decode_interval_ms,
        "last_decode_wall_ms": input.last_decode_wall_ms,
        "segment_enqueue_delta_ms": realtime.segment_enqueue_delta_ms,
        "silence_hold_ms": realtime.silence_hold_ms,
        "finalization_hold_ms": realtime.finalization_hold_ms,
        "vad_enabled": input.config.vad.enabled,
        "speech_threshold": input.config.vad.speech_threshold,
        "min_speech_ms": realtime.min_speech_ms,
        "min_silence_ms": realtime.finalization_hold_ms,
        "speech_pad_ms": input.config.vad.speech_pad_ms,
        "max_segment_ms": input.config.vad.max_segment_ms,
        "input_gain": input.config.recognition.input_gain,
        "preemphasis_enabled": input.config.recognition.preemphasis_enabled,
        "noise_gate_enabled": input.config.recognition.noise_gate_enabled,
        "hallucination_filter_enabled": input.config.recognition.hallucination_filter_enabled,
        "model_loaded": input.inference.model_loaded,
        "model_load_ms": input.inference.model_load_ms,
        "probe_cpu_ok": input.inference.probe_cpu_ok,
        "probe_cuda_ok": input.inference.probe_cuda_ok,
        "last_decode_timing": input.inference.last_decode_timing,
        "decode_count": input.decode_count,
        "finalized_segments": input.finalized_segments,
        "partial_emits": partial_emits,
        "final_emits": final_emits,
        "revision_emits": tel.map(|t| t.revision_emits).unwrap_or(0),
        "revision_rate": tel.map(|t| t.revision_rate).unwrap_or(0.0),
        "last_first_partial_ms": tel.and_then(|t| t.last_first_partial_ms),
        "last_final_ms": tel.and_then(|t| t.last_final_ms),
        "cpu_deps_ready": input.env.cpu_deps_ready,
        "cuda_deps_ready": input.env.cuda_deps_ready,
    })
}

pub fn partial_emit_from_config(config: &LocalAsrConfig) -> PartialEmitSettings {
    PartialEmitSettings::from_fields(
        &config.realtime.partial_emit_mode,
        config.realtime.partial_min_new_words,
        config.realtime.partial_min_delta_chars,
        config.realtime.partial_coalescing_ms,
    )
}

fn diagnostics_message(input: &LocalAsrDiagnosticsInput<'_>) -> String {
    if !input.env.vcruntime.ok {
        return "Install VC++ runtime.".into();
    }
    if !crate::setup::deps_ready_for_provider(input.env, &input.config.inference.execution_provider)
    {
        if input.config.inference.execution_provider == "cuda" {
            return "Complete GPU dependencies.".into();
        }
        return "Download ONNX Runtime (CPU or GPU).".into();
    }
    if !crate::setup::setup_is_valid(input.config, &input.config.inference.execution_provider) {
        return "Complete the one-time Local ASR setup checklist.".into();
    }
    if input.inference.model_loaded {
        format!(
            "Local Parakeet ready on {} EP.",
            input.inference.active_execution_provider
        )
    } else {
        format!(
            "Local Parakeet configured on {} EP — model loads on Live Start.",
            input.config.inference.execution_provider
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LocalAsrConfig;
    use crate::deps::env_check;
    use crate::emit_telemetry::EmitTelemetrySnapshot;
    use crate::inference::InferenceSnapshot;

    #[test]
    fn diagnostics_include_streaming_fields() {
        let dir = tempfile::tempdir().unwrap();
        let config = LocalAsrConfig::default();
        let tel = EmitTelemetrySnapshot {
            partial_emits: 4,
            final_emits: 1,
            revision_emits: 1,
            revision_rate: 0.25,
            last_first_partial_ms: Some(180),
            last_final_ms: Some(1200),
        };
        let body = assemble_local_asr_diagnostics(LocalAsrDiagnosticsInput {
            config: &config,
            env: &env_check(dir.path()),
            inference: &InferenceSnapshot {
                model_loaded: true,
                active_execution_provider: "cpu".into(),
                ..Default::default()
            },
            phase: LocalAsrModulePhase::Ready,
            is_runtime_running: true,
            decode_count: 3,
            finalized_segments: 1,
            emit_telemetry: Some(&tel),
            last_paced_decode_interval_ms: 0,
            last_decode_wall_ms: 0,
        });
        assert_eq!(body["true_streaming"], true);
        assert_eq!(body["partial_emit_mode"], "word_growth");
        assert_eq!(body["provider_kind"], "local_onnx");
        assert_eq!(body["decode_count"], 3);
        assert_eq!(body["last_first_partial_ms"], 180);
        assert_eq!(body["revision_emits"], 1);
        assert_eq!(body["partial_emits"], 4);
    }
}
