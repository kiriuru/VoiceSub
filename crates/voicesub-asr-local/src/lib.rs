//! Local ASR sidecar module — ONNX Runtime deps, Parakeet TDT inference, readiness.

#![recursion_limit = "256"]

pub mod diagnostics;
pub mod emit_policy;
pub mod emit_telemetry;
pub mod hallucination_filter;
pub mod local_asr_constants;
pub mod recognition_processing;
pub mod segment_enqueue;
pub mod segment_state;
pub mod vad_engine;
pub mod vad_tuning;
mod asr_segment_queue;
mod async_decode;
mod capture;
mod config;
mod decode;
mod decode_pacing;
mod decode_timing;
mod deps;
mod inference;
mod model_family;
mod model_manager;
mod pipeline;
mod realtime_settings;
mod runtime_session;
mod service;
mod setup;
mod status;
mod test_session;
mod transfer;

pub use capture::{CaptureError, InputDeviceInfo, MicStream, list_input_devices, record_input, start_mic_stream};
pub use config::{
    LocalAsrConfig, LocalAsrConfigError, LocalAsrConfigStore, LocalAsrRecognitionConfig,
    LocalAsrVadConfig, EXECUTION_PROVIDER_CPU, EXECUTION_PROVIDER_CUDA,
};
pub use deps::{
    DepDownloadKind, LocalAsrEnvCheck, CUDA_TOOLKIT_URL, delete_dependency, download_dependency,
    env_check, ort_dll_path_for_provider, prepare_ort_runtime, runtime_layout,
};
pub use inference::{InferenceEngine, InferenceError, InferenceSnapshot, LoadResult, ProbeResult};
pub use model_family::{
    model_display_label, normalize_model_selection, ModelFamily, FAMILY_PARAKEET_TDT, MODEL_FAMILY,
};
pub use model_manager::{
    ModelCatalogEntry, ModelError, ModelManifest, ModelVariant, MODEL_VARIANT_FP32,
    MODEL_VARIANT_INT8, MODEL_VARIANT_INT8_SMOOTHQUANT, build_all_model_catalogs,
    delete_model_variant, download_model, is_model_installed_at, load_manifest,
    model_dir_for_family_variant, model_dir_for_variant, resolve_model_dir,
};
pub use diagnostics::{assemble_local_asr_diagnostics, partial_emit_from_config, LocalAsrDiagnosticsInput};
pub use decode_pacing::{adaptive_partial_decode_interval_ms, max_segment_ms_for_preset};
pub use decode_timing::DecodeTimingBreakdown;
pub use emit_telemetry::{EmitTelemetry, EmitTelemetrySnapshot};
pub use local_asr_constants::SHORT_HALLUCINATION_TOKENS;
pub use pipeline::PipelineEmit;
pub use runtime_session::{LocalAsrRuntimeSession, RuntimeEmitCallback, RuntimeSessionError};
pub use service::LocalAsrModuleService;
pub use status::{LocalAsrModulePhase, LocalAsrModuleStatus, LocalAsrSetupChecklist};
pub use test_session::{TestBenchError, TestBenchPhase, TestBenchSnapshot};
pub use transfer::{TransferCancelled, TransferPhase, TransferProgress, TransferTracker};

pub const LOCAL_ASR_WINDOW_LABEL: &str = "local-asr";

pub fn build_local_asr_module_url(bind_addr: std::net::SocketAddr) -> String {
    if bind_addr.ip().is_unspecified() || bind_addr.ip().is_loopback() {
        format!("http://localhost:{}/local-asr", bind_addr.port())
    } else {
        format!(
            "http://{}:{}/local-asr",
            bind_addr.ip(),
            bind_addr.port()
        )
    }
}
