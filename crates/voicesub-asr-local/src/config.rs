use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info};

pub const EXECUTION_PROVIDER_CPU: &str = "cpu";
pub const EXECUTION_PROVIDER_CUDA: &str = "cuda";

#[derive(Debug, Error)]
pub enum LocalAsrConfigError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("toml serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalAsrModelConfig {
    #[serde(default = "default_model_variant")]
    pub variant: String,
    #[serde(default = "default_model_family")]
    pub family: String,
    /// Legacy config field (ignored). Kept for sidecar config compatibility.
    #[serde(default = "default_target_lang", alias = "targetLang")]
    pub target_lang: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub manifest_sha256: String,
}

impl Default for LocalAsrModelConfig {
    fn default() -> Self {
        Self {
            variant: default_model_variant(),
            family: default_model_family(),
            target_lang: default_target_lang(),
            path: String::new(),
            manifest_sha256: String::new(),
        }
    }
}

fn default_model_variant() -> String {
    "int8".into()
}

fn default_model_family() -> String {
    crate::model_family::MODEL_FAMILY.into()
}

fn default_target_lang() -> String {
    "auto".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalAsrInferenceConfig {
    #[serde(default = "default_execution_provider", alias = "executionProvider")]
    pub execution_provider: String,
    #[serde(default = "default_graph_opt_level", alias = "graphOptimizationLevel")]
    pub graph_optimization_level: u8,
    /// Intra-op thread pool size (ORT `intra_op_num_threads`). Matches `parakeet-rs` default of 4.
    #[serde(default = "default_intra_op_threads", alias = "intraOpThreads")]
    pub intra_op_threads: u32,
    /// Inter-op thread pool size (ORT `inter_op_num_threads`). Matches `parakeet-rs` default of 1.
    #[serde(default = "default_inter_op_threads", alias = "interOpThreads")]
    pub inter_op_threads: u32,
    /// ORT parallel execution mode (`ORT_PARALLEL` when true).
    #[serde(default = "default_false", alias = "parallelExecution")]
    pub parallel_execution: bool,
    /// Reuse memory allocation patterns across runs (best with stable input shapes).
    #[serde(default = "default_true", alias = "enableMemoryPattern")]
    pub enable_memory_pattern: bool,
    /// Write ORT Chrome-trace JSON under project `logs/ort-profile*` (flushed on unload).
    /// Streaming ASR emits many Session::run calls — without a decode budget files grow to 100MB+.
    #[serde(default = "default_false", alias = "ortProfiling")]
    pub ort_profiling: bool,
    /// Auto-unload after this many successful ONNX decodes while profiling (keeps JSON small).
    #[serde(default = "default_ort_profiling_max_decodes", alias = "ortProfilingMaxDecodes")]
    pub ort_profiling_max_decodes: u32,
    #[serde(default = "default_false", alias = "keepModelLoaded")]
    pub keep_model_loaded: bool,
    #[serde(default = "default_true", alias = "cudaFallbackToCpu")]
    pub cuda_fallback_to_cpu: bool,
}

impl Default for LocalAsrInferenceConfig {
    fn default() -> Self {
        Self {
            execution_provider: default_execution_provider(),
            graph_optimization_level: default_graph_opt_level(),
            intra_op_threads: default_intra_op_threads(),
            inter_op_threads: default_inter_op_threads(),
            parallel_execution: false,
            enable_memory_pattern: true,
            ort_profiling: false,
            ort_profiling_max_decodes: default_ort_profiling_max_decodes(),
            keep_model_loaded: false,
            cuda_fallback_to_cpu: true,
        }
    }
}

fn default_execution_provider() -> String {
    EXECUTION_PROVIDER_CPU.into()
}

fn default_graph_opt_level() -> u8 {
    1
}

fn default_intra_op_threads() -> u32 {
    4
}

fn default_inter_op_threads() -> u32 {
    1
}

/// Keep ORT Chrome-trace files small: a few Parakeet TDT decodes already produce multi-MB JSON.
fn default_ort_profiling_max_decodes() -> u32 {
    3
}

fn default_false() -> bool {
    false
}

fn default_true() -> bool {
    true
}

/// Clamp ORT session tunables to safe ranges used by the module UI.
pub fn normalize_inference_session_options(config: &mut LocalAsrInferenceConfig) {
    config.graph_optimization_level = config.graph_optimization_level.min(3);
    config.intra_op_threads = config.intra_op_threads.clamp(1, 64);
    config.inter_op_threads = config.inter_op_threads.clamp(1, 64);
    config.ort_profiling_max_decodes = config.ort_profiling_max_decodes.clamp(1, 50);
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalAsrRealtimeConfig {
    #[serde(default = "default_latency_preset", alias = "latencyPreset")]
    pub latency_preset: String,
    #[serde(default = "default_true", alias = "streamingDecode")]
    pub streaming_decode: bool,
    #[serde(default = "default_partial_emit_mode", alias = "partialEmitMode")]
    pub partial_emit_mode: String,
    #[serde(default = "default_one_u32", alias = "partialMinNewWords")]
    pub partial_min_new_words: u32,
    #[serde(default, alias = "partialMinDeltaChars")]
    pub partial_min_delta_chars: u32,
    #[serde(default, alias = "partialCoalescingMs")]
    pub partial_coalescing_ms: u32,
    /// Advanced override; `None` → latency preset default.
    #[serde(default, alias = "decodeIntervalMs")]
    pub decode_interval_ms: Option<u32>,
    #[serde(default, alias = "windowMs")]
    pub window_ms: Option<u32>,
    #[serde(default, alias = "segmentEnqueueDeltaMs")]
    pub segment_enqueue_delta_ms: Option<u32>,
    #[serde(default, alias = "firstPartialMinSpeechMs")]
    pub first_partial_min_speech_ms: Option<u32>,
}

impl Default for LocalAsrRealtimeConfig {
    fn default() -> Self {
        Self {
            latency_preset: default_latency_preset(),
            streaming_decode: true,
            partial_emit_mode: default_partial_emit_mode(),
            partial_min_new_words: default_one_u32(),
            partial_min_delta_chars: 0,
            partial_coalescing_ms: 0,
            decode_interval_ms: None,
            window_ms: None,
            segment_enqueue_delta_ms: None,
            first_partial_min_speech_ms: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalAsrVadConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_vad_mode", alias = "vadMode")]
    pub vad_mode: u8,
    #[serde(default = "default_true_optional", alias = "energyGateEnabled")]
    pub energy_gate_enabled: bool,
    #[serde(
        default = "default_min_rms_for_recognition",
        alias = "minRmsForRecognition"
    )]
    pub min_rms_for_recognition: f32,
    #[serde(default, alias = "minVoicedRatio")]
    pub min_voiced_ratio: f32,
    #[serde(default = "default_speech_attack_frames", alias = "speechAttackFrames")]
    pub speech_attack_frames: u32,
    #[serde(default = "default_speech_preroll_frames", alias = "speechPrerollFrames")]
    pub speech_preroll_frames: u32,
    #[serde(default, alias = "partialEmitIntervalMs")]
    pub partial_emit_interval_ms: Option<u32>,
    /// Legacy RMS threshold — maps to `min_rms_for_recognition` when unset.
    #[serde(default = "default_speech_threshold", alias = "speechThreshold")]
    pub speech_threshold: f32,
    #[serde(default = "default_min_speech_ms", alias = "minSpeechMs")]
    pub min_speech_ms: u32,
    #[serde(default = "default_min_silence_ms", alias = "minSilenceMs")]
    pub min_silence_ms: u32,
    #[serde(default = "default_silence_hold_ms", alias = "silenceHoldMs")]
    pub silence_hold_ms: u32,
    #[serde(default = "default_speech_pad_ms", alias = "speechPadMs")]
    pub speech_pad_ms: u32,
    #[serde(default = "default_max_segment_ms", alias = "maxSegmentMs")]
    pub max_segment_ms: u32,
}

impl Default for LocalAsrVadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            vad_mode: default_vad_mode(),
            energy_gate_enabled: false,
            min_rms_for_recognition: default_min_rms_for_recognition(),
            min_voiced_ratio: 0.0,
            speech_attack_frames: default_speech_attack_frames(),
            speech_preroll_frames: default_speech_preroll_frames(),
            partial_emit_interval_ms: None,
            speech_threshold: default_speech_threshold(),
            min_speech_ms: default_min_speech_ms(),
            min_silence_ms: default_min_silence_ms(),
            silence_hold_ms: default_silence_hold_ms(),
            speech_pad_ms: default_speech_pad_ms(),
            max_segment_ms: default_max_segment_ms(),
        }
    }
}

fn default_vad_mode() -> u8 {
    2
}

fn default_true_optional() -> bool {
    false
}

fn default_min_rms_for_recognition() -> f32 {
    0.0018
}

fn default_speech_attack_frames() -> u32 {
    2
}

fn default_speech_preroll_frames() -> u32 {
    5
}

fn default_speech_threshold() -> f32 {
    0.015
}

fn default_min_speech_ms() -> u32 {
    180
}

fn default_min_silence_ms() -> u32 {
    400
}

fn default_silence_hold_ms() -> u32 {
    180
}

fn default_speech_pad_ms() -> u32 {
    0
}

fn default_max_segment_ms() -> u32 {
    // Long Live monologues: finalize on silence (or 2 min safety), not every ~5 s.
    120_000
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalAsrRecognitionConfig {
    #[serde(default = "default_input_gain", alias = "inputGain")]
    pub input_gain: f32,
    #[serde(default, alias = "preemphasisEnabled")]
    pub preemphasis_enabled: bool,
    #[serde(default = "default_preemphasis_coeff", alias = "preemphasisCoeff")]
    pub preemphasis_coeff: f32,
    #[serde(default, alias = "noiseGateEnabled")]
    pub noise_gate_enabled: bool,
    #[serde(default = "default_noise_gate_threshold", alias = "noiseGateThreshold")]
    pub noise_gate_threshold: f32,
    #[serde(default = "default_true", alias = "hallucinationFilterEnabled")]
    pub hallucination_filter_enabled: bool,
    #[serde(default = "default_hallucination_min_chars", alias = "hallucinationMinChars")]
    pub hallucination_min_chars: u32,
    #[serde(default = "default_hallucination_cooldown_ms", alias = "hallucinationCooldownMs")]
    pub hallucination_cooldown_ms: u32,
}

impl Default for LocalAsrRecognitionConfig {
    fn default() -> Self {
        Self {
            input_gain: default_input_gain(),
            preemphasis_enabled: false,
            preemphasis_coeff: default_preemphasis_coeff(),
            noise_gate_enabled: false,
            noise_gate_threshold: default_noise_gate_threshold(),
            hallucination_filter_enabled: true,
            hallucination_min_chars: default_hallucination_min_chars(),
            hallucination_cooldown_ms: default_hallucination_cooldown_ms(),
        }
    }
}

fn default_input_gain() -> f32 {
    1.0
}

fn default_preemphasis_coeff() -> f32 {
    0.97
}

fn default_noise_gate_threshold() -> f32 {
    0.008
}

fn default_hallucination_min_chars() -> u32 {
    0
}

fn default_hallucination_cooldown_ms() -> u32 {
    500
}

fn default_latency_preset() -> String {
    "balanced".into()
}

fn default_partial_emit_mode() -> String {
    "word_growth".into()
}

fn default_one_u32() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalAsrMicrophoneConfig {
    #[serde(default, alias = "deviceId")]
    pub device_id: String,
    #[serde(default = "default_sample_rate", alias = "sampleRate")]
    pub sample_rate: u32,
}

impl Default for LocalAsrMicrophoneConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            sample_rate: default_sample_rate(),
        }
    }
}

fn default_sample_rate() -> u32 {
    16_000
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalAsrDepsConfig {
    #[serde(default = "default_ort_version")]
    pub onnxruntime_version: String,
    #[serde(default)]
    pub ort_cpu_installed: bool,
    #[serde(default)]
    pub ort_gpu_installed: bool,
    #[serde(default)]
    pub cuda_redist_installed: bool,
    #[serde(default)]
    pub last_env_check_at: String,
}

impl Default for LocalAsrDepsConfig {
    fn default() -> Self {
        Self {
            onnxruntime_version: default_ort_version(),
            ort_cpu_installed: false,
            ort_gpu_installed: false,
            cuda_redist_installed: false,
            last_env_check_at: String::new(),
        }
    }
}

fn default_ort_version() -> String {
    crate::deps::ORT_VERSION.into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LocalAsrSetupConfig {
    #[serde(default, alias = "setupComplete")]
    pub setup_complete: bool,
    #[serde(default, alias = "micTestPassed")]
    pub mic_test_passed: bool,
    #[serde(default, alias = "parakeetFinalReceived")]
    pub parakeet_final_received: bool,
    #[serde(default, alias = "validatedExecutionProvider")]
    pub validated_execution_provider: String,
    #[serde(default, alias = "completedAt")]
    pub completed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalAsrConfig {
    #[serde(default)]
    pub model: LocalAsrModelConfig,
    #[serde(default)]
    pub inference: LocalAsrInferenceConfig,
    #[serde(default)]
    pub realtime: LocalAsrRealtimeConfig,
    #[serde(default)]
    pub vad: LocalAsrVadConfig,
    #[serde(default)]
    pub recognition: LocalAsrRecognitionConfig,
    #[serde(default)]
    pub microphone: LocalAsrMicrophoneConfig,
    #[serde(default)]
    pub deps: LocalAsrDepsConfig,
    #[serde(default)]
    pub setup: LocalAsrSetupConfig,
}

impl LocalAsrConfig {
    fn patch_only_microphone(patch: &LocalAsrConfig) -> bool {
        patch.model.path.trim().is_empty()
            && patch.model.manifest_sha256.is_empty()
            && patch.inference == LocalAsrInferenceConfig::default()
            && patch.realtime == LocalAsrRealtimeConfig::default()
            && patch.vad == LocalAsrVadConfig::default()
            && patch.recognition == LocalAsrRecognitionConfig::default()
            && patch.microphone != LocalAsrMicrophoneConfig::default()
    }

    /// Merge fields from a partial API payload without wiping unspecified sections.
    /// Prefer [`Self::apply_tunable_from`] for dashboard `/config/save` payloads.
    pub fn merge_from(&mut self, patch: &LocalAsrConfig) {
        if !patch.model.path.trim().is_empty() || !patch.model.manifest_sha256.is_empty() {
            self.model = patch.model.clone();
        }
        if patch.inference != LocalAsrInferenceConfig::default() {
            self.inference = patch.inference.clone();
        } else if patch.inference.execution_provider != self.inference.execution_provider
            && !Self::patch_only_microphone(patch)
        {
            // CPU EP serializes as the inference default; still honor CUDA -> CPU downgrades.
            self.inference.execution_provider =
                normalize_execution_provider(&patch.inference.execution_provider);
        }
        if patch.realtime != LocalAsrRealtimeConfig::default() {
            self.realtime = patch.realtime.clone();
        }
        if patch.vad != LocalAsrVadConfig::default() {
            self.vad = patch.vad.clone();
        }
        if patch.recognition != LocalAsrRecognitionConfig::default() {
            self.recognition = patch.recognition.clone();
        }
        if patch.microphone != LocalAsrMicrophoneConfig::default() {
            self.microphone = patch.microphone.clone();
        }
    }

    /// Apply all user-facing tunables from a module UI save payload.
    ///
    /// Unlike [`Self::merge_from`], default-valued sections are still written so
    /// clearing advanced overrides back to preset defaults persists correctly.
    pub fn apply_tunable_from(&mut self, patch: &LocalAsrConfig) {
        if !patch.model.path.trim().is_empty() || !patch.model.manifest_sha256.is_empty() {
            self.model = patch.model.clone();
        }
        self.inference = patch.inference.clone();
        self.realtime = patch.realtime.clone();
        self.vad = patch.vad.clone();
        self.recognition = patch.recognition.clone();
        self.microphone = patch.microphone.clone();
    }
}

impl Default for LocalAsrConfig {
    fn default() -> Self {
        Self {
            model: LocalAsrModelConfig::default(),
            inference: LocalAsrInferenceConfig::default(),
            realtime: LocalAsrRealtimeConfig::default(),
            vad: LocalAsrVadConfig::default(),
            recognition: LocalAsrRecognitionConfig::default(),
            microphone: LocalAsrMicrophoneConfig::default(),
            deps: LocalAsrDepsConfig::default(),
            setup: LocalAsrSetupConfig::default(),
        }
    }
}

pub fn normalize_execution_provider(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        EXECUTION_PROVIDER_CUDA => EXECUTION_PROVIDER_CUDA.into(),
        _ => EXECUTION_PROVIDER_CPU.into(),
    }
}

pub struct LocalAsrConfigStore {
    module_dir: PathBuf,
}

impl LocalAsrConfigStore {
    pub fn new(module_dir: impl Into<PathBuf>) -> Self {
        Self {
            module_dir: module_dir.into(),
        }
    }

    pub fn path(&self) -> PathBuf {
        self.module_dir.join("config.toml")
    }

    pub fn module_dir(&self) -> &Path {
        &self.module_dir
    }

    pub fn load(&self) -> Result<LocalAsrConfig, LocalAsrConfigError> {
        let path = self.path();
        if !path.is_file() {
            debug!(
                target: "voicesub.asr_local",
                path = %path.display(),
                "local asr config missing — using defaults"
            );
            return Ok(LocalAsrConfig::default());
        }
        let raw = fs::read_to_string(&path)?;
        let mut config: LocalAsrConfig = toml::from_str(&raw)?;
        config.inference.execution_provider =
            normalize_execution_provider(&config.inference.execution_provider);
        normalize_inference_session_options(&mut config.inference);
        let (family, variant) =
            crate::model_family::normalize_model_selection(&config.model.family, &config.model.variant);
        config.model.family = family;
        config.model.variant = variant;
        if config.model.target_lang.trim().is_empty() {
            config.model.target_lang = default_target_lang();
        }
        heal_model_path(&mut config, &self.module_dir);
        Ok(config)
    }

    pub fn save(&self, config: &LocalAsrConfig) -> Result<(), LocalAsrConfigError> {
        fs::create_dir_all(&self.module_dir)?;
        let path = self.path();
        let tmp = path.with_extension("toml.tmp");
        let mut normalized = config.clone();
        normalized.inference.execution_provider =
            normalize_execution_provider(&normalized.inference.execution_provider);
        normalize_inference_session_options(&mut normalized.inference);
        let (family, variant) = crate::model_family::normalize_model_selection(
            &normalized.model.family,
            &normalized.model.variant,
        );
        normalized.model.family = family;
        normalized.model.variant = variant;
        if normalized.model.target_lang.trim().is_empty() {
            normalized.model.target_lang = default_target_lang();
        }
        heal_model_path(&mut normalized, &self.module_dir);
        let body = toml::to_string_pretty(&normalized)?;
        fs::write(&tmp, body)?;
        fs::rename(tmp, &path)?;
        info!(
            target: "voicesub.asr_local",
            path = %path.display(),
            provider = %normalized.inference.execution_provider,
            "saved local asr module config"
        );
        Ok(())
    }
}

/// Keep `model.path` aligned with the selected family/variant install dir.
/// Prevents stale paths (e.g. fp32 folder while variant is int8_smoothquant)
/// from surviving select→save roundtrips in the module UI.
fn heal_model_path(config: &mut LocalAsrConfig, module_dir: &Path) {
    use crate::model_family::ModelFamily;
    use crate::model_manager::{
        is_model_installed_for, load_manifest, model_dir_for_family_variant,
    };

    let family = ModelFamily::parse(&config.model.family).unwrap_or(ModelFamily::ParakeetTdt);
    let model_dir = model_dir_for_family_variant(module_dir, family, &config.model.variant);
    if is_model_installed_for(&model_dir, family, &config.model.variant) {
        let next = model_dir.display().to_string();
        if config.model.path != next {
            config.model.path = next;
        }
        if config.model.manifest_sha256.is_empty() {
            if let Some(manifest) = load_manifest(&model_dir) {
                config.model.manifest_sha256 = manifest.folder_sha256;
            }
        }
    } else if !config.model.path.trim().is_empty()
        && !is_model_installed_for(Path::new(config.model.path.trim()), family, &config.model.variant)
    {
        config.model.path.clear();
        config.model.manifest_sha256.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_session_thread_bounds() {
        let mut inf = LocalAsrInferenceConfig::default();
        inf.intra_op_threads = 0;
        inf.inter_op_threads = 99;
        inf.graph_optimization_level = 9;
        normalize_inference_session_options(&mut inf);
        assert_eq!(inf.intra_op_threads, 1);
        assert_eq!(inf.inter_op_threads, 64);
        assert_eq!(inf.graph_optimization_level, 3);
    }

    #[test]
    fn normalizes_execution_provider() {
        assert_eq!(normalize_execution_provider("CUDA"), EXECUTION_PROVIDER_CUDA);
        assert_eq!(normalize_execution_provider("unknown"), EXECUTION_PROVIDER_CPU);
    }

    #[test]
    fn merge_from_applies_cpu_downgrade() {
        let mut base = LocalAsrConfig::default();
        base.inference.execution_provider = EXECUTION_PROVIDER_CUDA.into();
        let patch = LocalAsrConfig {
            inference: LocalAsrInferenceConfig::default(),
            ..LocalAsrConfig::default()
        };
        base.merge_from(&patch);
        assert_eq!(base.inference.execution_provider, EXECUTION_PROVIDER_CPU);
    }

    #[test]
    fn merge_from_preserves_inference_when_patch_only_microphone() {
        let mut base = LocalAsrConfig::default();
        base.model.path = "/models/parakeet".into();
        base.inference.execution_provider = EXECUTION_PROVIDER_CUDA.into();
        let patch = LocalAsrConfig {
            microphone: LocalAsrMicrophoneConfig {
                device_id: "mic-1".into(),
                ..LocalAsrMicrophoneConfig::default()
            },
            ..LocalAsrConfig::default()
        };
        base.merge_from(&patch);
        assert_eq!(base.model.path, "/models/parakeet");
        assert_eq!(base.inference.execution_provider, EXECUTION_PROVIDER_CUDA);
        assert_eq!(base.microphone.device_id, "mic-1");
    }

    #[test]
    fn deserializes_execution_provider_alias() {
        let raw = r#"
[inference]
executionProvider = "cuda"
"#;
        let config: LocalAsrConfig = toml::from_str(raw).unwrap();
        assert_eq!(config.inference.execution_provider, EXECUTION_PROVIDER_CUDA);
    }

    #[test]
    fn deserializes_realtime_camel_case_from_api() {
        let raw = r#"{
            "realtime": {
                "latencyPreset": "balanced",
                "streamingDecode": false,
                "partialEmitMode": "char_delta",
                "partialMinNewWords": 3,
                "partialMinDeltaChars": 5,
                "partialCoalescingMs": 120,
                "decodeIntervalMs": 500,
                "windowMs": 4000,
                "segmentEnqueueDeltaMs": 80
            }
        }"#;
        let patch: LocalAsrConfig = serde_json::from_str(raw).unwrap();
        assert_eq!(patch.realtime.latency_preset, "balanced");
        assert!(!patch.realtime.streaming_decode);
        assert_eq!(patch.realtime.partial_emit_mode, "char_delta");
        assert_eq!(patch.realtime.partial_min_new_words, 3);
        assert_eq!(patch.realtime.partial_min_delta_chars, 5);
        assert_eq!(patch.realtime.partial_coalescing_ms, 120);
        assert_eq!(patch.realtime.decode_interval_ms, Some(500));
        assert_eq!(patch.realtime.window_ms, Some(4000));
        assert_eq!(patch.realtime.segment_enqueue_delta_ms, Some(80));

        let mut base = LocalAsrConfig::default();
        base.apply_tunable_from(&patch);
        assert_eq!(base.realtime.latency_preset, "balanced");
        assert!(!base.realtime.streaming_decode);
    }

    #[test]
    fn apply_tunable_from_writes_default_realtime_over_custom() {
        let mut base = LocalAsrConfig::default();
        base.realtime.latency_preset = "quality".into();
        base.realtime.window_ms = Some(6000);
        let patch = LocalAsrConfig::default();
        base.apply_tunable_from(&patch);
        assert_eq!(base.realtime.latency_preset, "balanced");
        assert_eq!(base.realtime.window_ms, None);
    }

    #[test]
    fn roundtrip_config_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalAsrConfigStore::new(dir.path());
        let mut config = LocalAsrConfig::default();
        config.inference.execution_provider = EXECUTION_PROVIDER_CUDA.into();
        store.save(&config).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.inference.execution_provider, EXECUTION_PROVIDER_CUDA);
    }
}
