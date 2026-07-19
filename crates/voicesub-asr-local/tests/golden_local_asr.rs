use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use voicesub_asr_local::diagnostics::{LocalAsrDiagnosticsInput, assemble_local_asr_diagnostics};
use voicesub_asr_local::emit_policy::RealtimeEmitPolicy;
use voicesub_asr_local::hallucination_filter::{
    HallucinationFilter, HallucinationFilterConfig, should_drop_short_hallucination,
};
use voicesub_asr_local::recognition_processing::RecognitionProcessor;
use voicesub_asr_local::segment_enqueue::SegmentAudioEnqueue;
use voicesub_asr_local::{
    InferenceSnapshot, LocalAsrConfig, LocalAsrModulePhase, LocalAsrRecognitionConfig,
    PipelineEmit, env_check,
};

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join("tests/golden/local_asr")
        .join(name)
}

fn load_fixture(name: &str) -> Value {
    let raw = fs::read_to_string(golden_path(name)).expect("read fixture");
    serde_json::from_str(&raw).expect("parse fixture")
}

#[derive(Debug, Deserialize)]
struct EnqueueStep {
    push_samples: usize,
    speech_active: bool,
    buffer_samples: usize,
    expect_decode: bool,
}

#[test]
fn golden_segment_enqueue_delta() {
    let fixture = load_fixture("segment_enqueue_delta.json");
    let sample_rate = fixture["sample_rate"].as_u64().unwrap() as u32;
    let mut queue = SegmentAudioEnqueue::new(
        fixture["delta_ms"].as_u64().unwrap() as u32,
        fixture["decode_interval_ms"].as_u64().unwrap() as u32,
        sample_rate,
        fixture["min_buffer_ms"].as_u64().unwrap() as u32,
    );
    thread::sleep(Duration::from_millis(450));
    let steps: Vec<EnqueueStep> = serde_json::from_value(fixture["steps"].clone()).expect("steps");
    for step in steps {
        queue.push_samples(step.push_samples);
        assert_eq!(
            queue.should_decode(step.buffer_samples, step.speech_active),
            step.expect_decode,
            "push={} buffer={}",
            step.push_samples,
            step.buffer_samples
        );
    }
}

#[derive(Debug, Deserialize)]
struct ShortHallucinationCase {
    text: String,
    duration_ms: u64,
    is_final: bool,
    expect_drop: bool,
}

#[test]
fn golden_short_hallucination_tokens() {
    let fixture = load_fixture("short_hallucination_tokens.json");
    let tokens: Vec<String> = serde_json::from_value(fixture["tokens"].clone()).expect("tokens");
    assert_eq!(
        tokens.len(),
        fixture["token_count"].as_u64().unwrap() as usize,
        "fixture token_count mismatch"
    );
    assert_eq!(
        tokens.len(),
        voicesub_asr_local::SHORT_HALLUCINATION_TOKENS.len(),
        "rust token list drift vs golden fixture"
    );
    for token in &tokens {
        assert!(
            should_drop_short_hallucination(token, 500, false),
            "token {token:?} should drop within partial limit"
        );
        assert!(
            !should_drop_short_hallucination(token, 2000, false),
            "token {token:?} should keep beyond partial limit"
        );
    }
    let cases: Vec<ShortHallucinationCase> =
        serde_json::from_value(fixture["cases"].clone()).expect("cases");
    for case in cases {
        assert_eq!(
            should_drop_short_hallucination(&case.text, case.duration_ms as u32, case.is_final,),
            case.expect_drop,
            "text={:?} duration_ms={}",
            case.text,
            case.duration_ms,
        );
    }
}

#[derive(Debug, Deserialize)]
struct HallucinationCase {
    text: String,
    speech_active: bool,
    expect_accept: bool,
}

#[test]
fn golden_hallucination_filter_silence() {
    let fixture = load_fixture("hallucination_filter_silence.json");
    let cfg_raw = &fixture["config"];
    let config = HallucinationFilterConfig {
        enabled: cfg_raw["enabled"].as_bool().unwrap(),
        min_chars_when_silent: cfg_raw["min_chars_when_silent"].as_u64().unwrap() as u32,
        cooldown_ms: cfg_raw["cooldown_ms"].as_u64().unwrap() as u32,
    };
    let mut filter = HallucinationFilter::new(config);
    let cases: Vec<HallucinationCase> =
        serde_json::from_value(fixture["cases"].clone()).expect("cases");
    for case in cases {
        assert_eq!(
            filter.accept(&case.text, case.speech_active),
            case.expect_accept,
            "text={:?} speech={}",
            case.text,
            case.speech_active
        );
    }
}

#[derive(Debug, Deserialize)]
struct EmitPolicyStep {
    segment_id: String,
    text: String,
    is_final: bool,
    expect_emit: bool,
}

#[test]
fn golden_emit_policy_dedup_partial() {
    let fixture = load_fixture("emit_policy_dedup.json");
    let mut policy = RealtimeEmitPolicy::default();
    let steps: Vec<EmitPolicyStep> =
        serde_json::from_value(fixture["steps"].clone()).expect("steps");
    for step in steps {
        let out = policy.apply(vec![PipelineEmit {
            segment_id: step.segment_id.clone(),
            revision: 1,
            text: step.text.clone(),
            is_final: step.is_final,
            is_speech: !step.is_final,
        }]);
        assert_eq!(
            !out.is_empty(),
            step.expect_emit,
            "segment={} text={:?} final={}",
            step.segment_id,
            step.text,
            step.is_final
        );
    }
}

#[test]
fn golden_recognition_processing_gain() {
    let fixture = load_fixture("recognition_processing_gain.json");
    let cfg_raw = &fixture["config"];
    let processor = RecognitionProcessor::new(LocalAsrRecognitionConfig {
        input_gain: cfg_raw["input_gain"].as_f64().unwrap() as f32,
        preemphasis_enabled: cfg_raw["preemphasis_enabled"].as_bool().unwrap(),
        noise_gate_enabled: cfg_raw["noise_gate_enabled"].as_bool().unwrap(),
        ..LocalAsrRecognitionConfig::default()
    });
    let input: Vec<f32> = fixture["input"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect();
    let expected: Vec<f32> = fixture["expected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect();
    let mut samples = input;
    processor.process_in_place(&mut samples);
    assert_eq!(samples, expected);
}

#[test]
fn golden_diagnostics_snapshot() {
    let fixture = load_fixture("diagnostics_snapshot.json");
    let expected = &fixture["expected"];
    let dir = tempfile::tempdir().unwrap();
    let config = LocalAsrConfig::default();
    let body = assemble_local_asr_diagnostics(LocalAsrDiagnosticsInput {
        config: &config,
        env: &env_check(dir.path()),
        inference: &InferenceSnapshot {
            model_loaded: expected["model_loaded"].as_bool().unwrap(),
            active_execution_provider: "cpu".into(),
            ..Default::default()
        },
        phase: LocalAsrModulePhase::Ready,
        is_runtime_running: true,
        decode_count: expected["decode_count"].as_u64().unwrap(),
        finalized_segments: expected["finalized_segments"].as_u64().unwrap(),
        emit_telemetry: None,
        last_paced_decode_interval_ms: 0,
        last_decode_wall_ms: 0,
    });
    for (key, value) in expected.as_object().unwrap() {
        assert_eq!(&body[key], value, "field {key}");
    }
}

#[test]
fn golden_vad_engine_partial_final() {
    use std::f32::consts::PI;
    use voicesub_asr_local::vad_engine::{
        VadEngine, VadEngineConfig, VadSegmentKind, f32_to_pcm_bytes,
    };

    let fixture = load_fixture("vad_engine_partial_final.json");
    let cfg_raw = &fixture["config"];
    let config = VadEngineConfig {
        mode: cfg_raw["vad_mode"].as_u64().unwrap() as u8,
        energy_gate_enabled: cfg_raw["energy_gate_enabled"].as_bool().unwrap(),
        min_rms_for_recognition: cfg_raw["min_rms_for_recognition"].as_f64().unwrap() as f32,
        min_voiced_ratio: cfg_raw["min_voiced_ratio"].as_f64().unwrap() as f32,
        speech_attack_frames: cfg_raw["speech_attack_frames"].as_u64().unwrap() as u32,
        speech_preroll_frames: cfg_raw["speech_preroll_frames"].as_u64().unwrap() as u32,
        min_speech_ms: cfg_raw["min_speech_ms"].as_u64().unwrap() as u32,
        finalization_hold_ms: cfg_raw["min_silence_ms"].as_u64().unwrap() as u32,
        silence_hold_ms: cfg_raw["silence_hold_ms"].as_u64().unwrap() as u32,
        partial_emit_interval_ms: cfg_raw["partial_emit_interval_ms"].as_u64().unwrap() as u32,
        max_segment_ms: cfg_raw["max_segment_ms"].as_u64().unwrap() as u32,
        first_partial_min_speech_ms: cfg_raw["min_speech_ms"].as_u64().unwrap() as u32,
        ..VadEngineConfig::default()
    };
    let frame_samples = fixture["frame_samples"].as_u64().unwrap() as usize;
    let speech_level = fixture["speech_level"].as_f64().unwrap() as f32;
    let speech_frame = f32_to_pcm_bytes(
        &(0..frame_samples)
            .map(|idx| (2.0 * PI * 300.0 * idx as f32 / 16_000.0).sin() * speech_level)
            .collect::<Vec<_>>(),
    );

    let mut vad = VadEngine::new(config);
    let mut saw_partial = false;
    for _ in 0..fixture["speech_frames"].as_u64().unwrap() {
        for segment in vad.process_chunk(&speech_frame) {
            if segment.kind == VadSegmentKind::Partial {
                saw_partial = true;
            }
        }
    }
    let finals = vad.force_finalize();
    assert_eq!(saw_partial, fixture["expect_partial"].as_bool().unwrap());
    assert_eq!(
        finals
            .iter()
            .any(|segment| segment.kind == VadSegmentKind::Final),
        fixture["expect_final"].as_bool().unwrap()
    );
}

#[test]
fn golden_segment_audio_delta_slice() {
    use voicesub_asr_local::segment_enqueue::slice_segment_audio_delta;

    let mut tracker = std::collections::HashMap::new();
    let full = vec![1.0, 2.0, 3.0, 4.0];
    let (delta, skip) = slice_segment_audio_delta(&full, "seg-1", true, &mut tracker);
    assert!(!skip);
    assert_eq!(delta, full);
    let (delta2, skip2) = slice_segment_audio_delta(&full, "seg-1", false, &mut tracker);
    assert!(skip2);
    assert!(delta2.is_empty());
    let extended = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let (delta3, skip3) = slice_segment_audio_delta(&extended, "seg-1", false, &mut tracker);
    assert!(!skip3);
    assert_eq!(delta3, vec![5.0]);
}
