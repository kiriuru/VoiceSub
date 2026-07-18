use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use tracing::{info, warn};

use crate::config::{LocalAsrConfig, LocalAsrConfigStore};
use crate::deps::{DepDownloadKind, DepError, cleanup_pending_runtime_removals, delete_dependency, download_dependency, env_check, validate_deps_for_provider};
use crate::diagnostics::{assemble_local_asr_diagnostics, LocalAsrDiagnosticsInput};
use crate::inference::{InferenceEngine, InferenceError, InferenceSnapshot, LoadResult, ProbeResult};
use crate::model_family::ModelFamily;
use crate::model_manager::{self, delete_model_variant, download_model, is_model_installed_for, model_dir_for_family_variant, ModelError};
use crate::realtime_settings::{apply_latency_preset_to_config, normalize_latency_preset};
use crate::runtime_session::{LocalAsrRuntimeSession, RuntimeEmitCallback, RuntimeSessionError};
use crate::setup::{apply_test_bench_progress, clear_setup, try_finalize_setup};
use crate::status::{LocalAsrModulePhase, LocalAsrModuleStatus, build_status};
use crate::test_session::{TestBench, TestBenchError, TestBenchPhase, TestBenchSnapshot};
use crate::transfer::{TransferProgress, TransferTracker};

pub struct LocalAsrModuleService {
    store: LocalAsrConfigStore,
    logs_dir: PathBuf,
    cached_status: RwLock<Option<LocalAsrModuleStatus>>,
    transfer: TransferTracker,
    inference: Arc<InferenceEngine>,
    test_bench: TestBench,
    runtime_session: LocalAsrRuntimeSession,
}

impl LocalAsrModuleService {
    pub fn new(user_data_dir: impl Into<PathBuf>, logs_dir: impl Into<PathBuf>) -> Self {
        let module_dir = user_data_dir.into().join("modules").join("local-asr");
        let logs_dir = logs_dir.into();
        info!(
            target: "voicesub.asr_local",
            path = %module_dir.display(),
            logs = %logs_dir.display(),
            "local asr module service initialized"
        );
        cleanup_pending_runtime_removals(&module_dir);
        model_manager::cleanup_pending_model_removals(&module_dir);
        // Do not run env_check / ORT init here: CUDA Toolkit `bin/` scans and ORT load
        // block VoiceSub HTTP readiness (dashboard theme/settings appear seconds late).
        // GPU ORT initializes lazily on first probe/load/status refresh.
        Self {
            store: LocalAsrConfigStore::new(module_dir),
            logs_dir,
            cached_status: RwLock::new(None),
            transfer: TransferTracker::new(),
            inference: Arc::new(InferenceEngine::new()),
            test_bench: TestBench::new(),
            runtime_session: LocalAsrRuntimeSession::new(),
        }
    }

    pub fn module_dir(&self) -> &Path {
        self.store.module_dir()
    }

    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }

    pub fn config_path(&self) -> PathBuf {
        self.store.path()
    }

    pub fn transfer_snapshot(&self) -> TransferProgress {
        self.transfer.snapshot()
    }

    pub fn inference_snapshot(&self) -> InferenceSnapshot {
        self.inference.snapshot()
    }

    pub fn test_bench_snapshot(&self) -> TestBenchSnapshot {
        let snap = self.test_bench.snapshot();
        if !snap.running && snap.phase == TestBenchPhase::Done {
            let _ = self.commit_setup_from_test(&snap);
        }
        snap
    }

    pub fn diagnostics(&self) -> serde_json::Value {
        // Heartbeat calls this every ~1s — reuse status()/env cache instead of env_check.
        let status = self.status();
        let config = self.load_config().unwrap_or_default();
        let inference = self.inference.snapshot();
        let test = self.test_bench.snapshot();
        let runtime_tel = self.runtime_session.emit_telemetry();
        let test_tel = self.test_bench.emit_telemetry();
        let tel = if self.runtime_session.is_running() || runtime_tel.partial_emits > 0 {
            runtime_tel
        } else {
            test_tel
        };
        let is_runtime_running = test.running || self.runtime_session.is_running();
        let phase = if is_runtime_running {
            LocalAsrModulePhase::Running
        } else {
            status.phase
        };
        assemble_local_asr_diagnostics(LocalAsrDiagnosticsInput {
            config: &config,
            env: &status.env,
            inference: &inference,
            phase,
            is_runtime_running,
            decode_count: u64::from(test.decode_count).max(tel.partial_emits),
            finalized_segments: u64::from(test.finalized_segments).max(tel.final_emits),
            emit_telemetry: Some(&tel),
            last_paced_decode_interval_ms: 0,
            last_decode_wall_ms: inference
                .last_decode_timing
                .as_ref()
                .map(|t| t.total_ms())
                .unwrap_or(0),
        })
    }

    pub fn load_config(&self) -> Result<LocalAsrConfig, crate::config::LocalAsrConfigError> {
        self.store.load()
    }

    pub fn save_config(&self, config: &LocalAsrConfig) -> Result<(), crate::config::LocalAsrConfigError> {
        let mut merged = self.load_config().unwrap_or_default();
        let previous_provider = merged.inference.execution_provider.clone();
        let previous_session = session_options_fingerprint(&merged.inference);
        let previous_preset = merged.realtime.latency_preset.clone();
        merged.apply_tunable_from(config);
        merged.inference.execution_provider = crate::config::normalize_execution_provider(
            &merged.inference.execution_provider,
        );
        crate::config::normalize_inference_session_options(&mut merged.inference);
        if normalize_latency_preset(&merged.realtime.latency_preset)
            != normalize_latency_preset(&previous_preset)
        {
            apply_latency_preset_to_config(&mut merged);
        }
        if merged.inference.execution_provider != previous_provider {
            self.inference.unload();
            clear_setup(&mut merged);
        } else if session_options_fingerprint(&merged.inference) != previous_session {
            // ORT session options apply only on next warm load.
            self.inference.unload();
        }
        self.store.save(&merged)?;
        self.invalidate_status_cache();
        Ok(())
    }

    pub fn list_microphones(&self) -> Result<Vec<crate::capture::InputDeviceInfo>, crate::capture::CaptureError> {
        crate::capture::list_input_devices()
    }

    pub fn status(&self) -> LocalAsrModuleStatus {
        if let Ok(guard) = self.cached_status.read()
            && let Some(cached) = guard.as_ref()
        {
            return cached.clone();
        }
        self.refresh_status()
    }

    pub fn refresh_status(&self) -> LocalAsrModuleStatus {
        let config = self.load_config().unwrap_or_default();
        let env = env_check(self.store.module_dir());
        let inference = self.inference.snapshot();
        let status = build_status(&config, env, self.store.module_dir(), &inference);
        if let Ok(mut guard) = self.cached_status.write() {
            *guard = Some(status.clone());
        }
        status
    }

    pub fn invalidate_status_cache(&self) {
        if let Ok(mut guard) = self.cached_status.write() {
            *guard = None;
        }
    }

    pub fn cancel_transfer(&self) -> TransferProgress {
        self.transfer.request_cancel();
        self.transfer.snapshot()
    }

    pub async fn download_deps(&self, kind: DepDownloadKind) -> Result<LocalAsrModuleStatus, DepError> {
        let mut reporter = self.transfer.reporter();
        match download_dependency(self.store.module_dir(), kind, &mut reporter).await {
            Ok(()) => {
                self.inference.unload();
                if matches!(kind, DepDownloadKind::OrtCpu | DepDownloadKind::OrtGpu) {
                    let _ = InferenceEngine::ensure_ort_initialized(self.store.module_dir());
                }
                self.invalidate_status_cache();
                Ok(self.refresh_status())
            }
            Err(DepError::Cancelled) => {
                reporter.finish_cancelled();
                self.invalidate_status_cache();
                Ok(self.refresh_status())
            }
            Err(err) => {
                reporter.finish_err(err.to_string());
                Err(err)
            }
        }
    }

    pub fn delete_deps(&self, kind: DepDownloadKind) -> Result<LocalAsrModuleStatus, DepError> {
        self.inference.unload();
        delete_dependency(self.store.module_dir(), kind)?;
        self.transfer.clear();
        let mut config = self.load_config().unwrap_or_default();
        clear_setup(&mut config);
        let _ = self.store.save(&config);
        self.invalidate_status_cache();
        Ok(self.refresh_status())
    }

    pub fn catalog_for_family(&self, family_raw: &str) -> Vec<model_manager::ModelCatalogEntry> {
        let config = self.load_config().unwrap_or_default();
        let active = if config.model.family.eq_ignore_ascii_case(family_raw) {
            config.model.variant.as_str()
        } else {
            ""
        };
        model_manager::build_model_catalog(self.store.module_dir(), family_raw, active)
    }

    pub async fn download_model(
        &self,
        family_raw: &str,
        variant: &str,
    ) -> Result<LocalAsrModuleStatus, model_manager::ModelError> {
        let family = ModelFamily::parse(family_raw).unwrap_or(ModelFamily::ParakeetTdt);
        let spec = family
            .parse_variant(variant)
            .ok_or_else(|| model_manager::ModelError::UnknownVariant(variant.to_string()))?;
        let mut reporter = self.transfer.reporter();
        let model_dir = match download_model(
            self.store.module_dir(),
            family.as_str(),
            spec.variant,
            &mut reporter,
        )
        .await
        {
            Ok(path) => path,
            Err(ModelError::Cancelled) => {
                reporter.finish_cancelled();
                self.invalidate_status_cache();
                return Ok(self.refresh_status());
            }
            Err(err) => {
                reporter.finish_err(err.to_string());
                return Err(err);
            }
        };
        let manifest = model_manager::load_manifest(&model_dir).ok_or_else(|| {
            model_manager::ModelError::Manifest("manifest missing after model download".into())
        })?;
        let mut config = self.load_config().unwrap_or_default();
        config.model.family = family.as_str().into();
        config.model.variant = spec.variant.into();
        config.model.path = model_dir.display().to_string();
        config.model.manifest_sha256 = manifest.folder_sha256;
        self.inference.unload();
        self.save_config(&config)
            .map_err(|e| model_manager::ModelError::Manifest(e.to_string()))?;
        self.invalidate_status_cache();
        Ok(self.refresh_status())
    }

    pub fn select_model(
        &self,
        family_raw: &str,
        variant: &str,
    ) -> Result<LocalAsrModuleStatus, model_manager::ModelError> {
        let family = ModelFamily::parse(family_raw).unwrap_or(ModelFamily::ParakeetTdt);
        let spec = family
            .parse_variant(variant)
            .ok_or_else(|| model_manager::ModelError::UnknownVariant(variant.to_string()))?;
        let mut config = self.load_config().unwrap_or_default();
        let selection_changed = !(config.model.family.eq_ignore_ascii_case(family.as_str())
            && config.model.variant.eq_ignore_ascii_case(spec.variant));
        config.model.family = family.as_str().into();
        config.model.variant = spec.variant.into();
        let model_dir = model_dir_for_family_variant(self.store.module_dir(), family, spec.variant);
        let (next_path, next_sha) = if is_model_installed_for(&model_dir, family, spec.variant) {
            (
                model_dir.display().to_string(),
                model_manager::load_manifest(&model_dir)
                    .map(|manifest| manifest.folder_sha256)
                    .unwrap_or_default(),
            )
        } else {
            (String::new(), String::new())
        };
        let path_healed =
            config.model.path != next_path || config.model.manifest_sha256 != next_sha;
        config.model.path = next_path;
        config.model.manifest_sha256 = next_sha;
        if !selection_changed && !path_healed {
            return Ok(self.refresh_status());
        }
        if selection_changed {
            self.inference.unload();
            clear_setup(&mut config);
        }
        self.store
            .save(&config)
            .map_err(|err| model_manager::ModelError::Manifest(err.to_string()))?;
        self.invalidate_status_cache();
        Ok(self.refresh_status())
    }

    pub fn delete_model(
        &self,
        family_raw: &str,
        variant: &str,
    ) -> Result<LocalAsrModuleStatus, model_manager::ModelError> {
        self.inference.unload();
        delete_model_variant(self.store.module_dir(), family_raw, variant)?;
        let mut config = self.load_config().unwrap_or_default();
        let deleted_active = config.model.family.eq_ignore_ascii_case(family_raw)
            && config.model.variant.eq_ignore_ascii_case(variant);
        if deleted_active {
            config.model.path.clear();
            config.model.manifest_sha256.clear();
            // Active selection pointed at a missing install — reset checklist.
            clear_setup(&mut config);
        }
        self.inference.unload();
        self.save_config(&config)
            .map_err(|e| model_manager::ModelError::Manifest(e.to_string()))?;
        self.transfer.clear();
        self.invalidate_status_cache();
        Ok(self.refresh_status())
    }

    pub fn probe_provider(&self, provider: &str) -> Result<ProbeResult, InferenceError> {
        let config = self.load_config().unwrap_or_default();
        let result = InferenceEngine::probe(self.store.module_dir(), &config, provider)?;
        self.inference.record_probe(&result);
        self.invalidate_status_cache();
        Ok(result)
    }

    pub fn load_model(&self) -> Result<LoadResult, InferenceError> {
        let config = self.load_config().unwrap_or_default();
        let result = self.inference.load(self.store.module_dir(), &self.logs_dir, &config)?;
        self.invalidate_status_cache();
        Ok(result)
    }

    pub fn unload_model(&self) -> LocalAsrModuleStatus {
        self.inference.unload();
        self.invalidate_status_cache();
        self.refresh_status()
    }

    /// Unload warm session after module test bench unless user opted into `keep_model_loaded`.
    pub fn unload_model_if_ephemeral(&self) {
        let config = self.load_config().unwrap_or_default();
        if config.inference.keep_model_loaded {
            return;
        }
        self.unload_model_after_runtime_stop();
    }

    /// SST runtime stop parity — always release ONNX session after Live stops.
    pub fn unload_model_after_runtime_stop(&self) {
        if self.inference.snapshot().model_loaded {
            self.inference.unload();
            self.invalidate_status_cache();
        }
    }

    pub fn runtime_capture_running(&self) -> bool {
        self.runtime_session.is_running()
    }

    pub fn start_runtime_capture(
        &self,
        on_emit: RuntimeEmitCallback,
    ) -> Result<(), RuntimeSessionError> {
        if self.test_bench.snapshot().running {
            return Err(RuntimeSessionError::Precondition(
                "stop the module test bench before starting runtime capture".into(),
            ));
        }
        if !self.inference.snapshot().model_loaded {
            return Err(RuntimeSessionError::Precondition(
                "load the model before starting runtime capture".into(),
            ));
        }
        let status = self.status();
        if !status.ready {
            return Err(RuntimeSessionError::Precondition(format!(
                "local ASR module is not ready: {}",
                status.message
            )));
        }
        let config = self.load_config().map_err(|err| {
            RuntimeSessionError::Precondition(format!("failed to load module config: {err}"))
        })?;
        self.runtime_session
            .start(Arc::clone(&self.inference), config, on_emit)
    }

    pub fn stop_runtime_capture(&self) -> Result<(), RuntimeSessionError> {
        self.runtime_session.stop()
    }

    pub fn start_test(&self, duration_ms: u64, device_id: Option<&str>) -> Result<TestBenchSnapshot, TestBenchError> {
        if self.runtime_session.is_running() {
            return Err(TestBenchError::Inference(InferenceError::Runtime(
                "stop runtime capture before running the test bench".into(),
            )));
        }
        if !self.inference.snapshot().model_loaded {
            return Err(TestBenchError::Inference(InferenceError::Runtime(
                "load the model before running the test bench".into(),
            )));
        }
        let mut config = self.load_config().unwrap_or_default();
        if let Some(id) = device_id {
            config.microphone.device_id = id.to_string();
            self.save_config(&config)
                .map_err(|err| TestBenchError::Inference(InferenceError::Runtime(err.to_string())))?;
        }
        let device_id = config.microphone.device_id.clone();
        let device_label = crate::capture::list_input_devices()
            .ok()
            .and_then(|devices| {
                devices
                    .iter()
                    .find(|entry| entry.id == device_id)
                    .map(|entry| entry.label.clone())
            })
            .unwrap_or_else(|| {
                if device_id.trim().is_empty() {
                    "Default".into()
                } else {
                    device_id.clone()
                }
            });
        let provider = self
            .inference
            .snapshot()
            .active_execution_provider
            .clone();
        self.test_bench.start(
            Arc::clone(&self.inference),
            config.clone(),
            duration_ms,
            provider,
            device_id,
            device_label,
        )?;
        Ok(self.test_bench.snapshot())
    }

    pub fn stop_test(&self) -> Result<TestBenchSnapshot, TestBenchError> {
        let snap = self.test_bench.stop()?;
        if let Err(err) = self.commit_setup_from_test(&snap) {
            warn!(
                target: "voicesub.asr_local",
                error = %err,
                "failed to persist setup checklist after mic test"
            );
        }
        self.unload_model_if_ephemeral();
        Ok(snap)
    }

    fn commit_setup_from_test(&self, snap: &TestBenchSnapshot) -> Result<(), crate::config::LocalAsrConfigError> {
        let previous = self.load_config().unwrap_or_default();
        let mut config = previous.clone();
        apply_test_bench_progress(&mut config, snap);
        let env = env_check(self.store.module_dir());
        try_finalize_setup(&mut config, &env, self.store.module_dir());
        if config.setup == previous.setup {
            return Ok(());
        }
        self.store.save(&config)?;
        if config.setup.setup_complete && !config.inference.keep_model_loaded {
            self.unload_model_if_ephemeral();
        }
        self.invalidate_status_cache();
        Ok(())
    }

    pub fn validate_current_deps(&self) -> Result<(), DepError> {
        let config = self.load_config().unwrap_or_default();
        let env = env_check(self.store.module_dir());
        validate_deps_for_provider(&env, &config.inference.execution_provider)
    }
}

fn session_options_fingerprint(
    inf: &crate::config::LocalAsrInferenceConfig,
) -> (u8, u32, u32, bool, bool, bool, u32) {
    (
        inf.graph_optimization_level,
        inf.intra_op_threads,
        inf.inter_op_threads,
        inf.parallel_execution,
        inf.enable_memory_pattern,
        inf.ort_profiling,
        inf.ort_profiling_max_decodes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_manager::{ModelVariant, MODEL_VARIANT_INT8, MODEL_VARIANT_FP32};

    #[test]
    fn unload_after_runtime_stop_is_idempotent_when_unloaded() {
        let dir = tempfile::tempdir().unwrap();
        let service = LocalAsrModuleService::new(dir.path(), dir.path().join("logs"));
        service.unload_model_after_runtime_stop();
        assert!(!service.inference_snapshot().model_loaded);
    }

    #[test]
    fn diagnostics_reuses_warm_status_cache() {
        let dir = tempfile::tempdir().unwrap();
        let service = LocalAsrModuleService::new(dir.path(), dir.path().join("logs"));
        let first = service.refresh_status();
        let diag = service.diagnostics();
        assert_eq!(
            diag.get("mode").and_then(|v| v.as_str()),
            Some("local_parakeet")
        );
        assert_eq!(
            diag.get("cpu_deps_ready").and_then(|v| v.as_bool()),
            Some(first.env.cpu_deps_ready)
        );
        // Second diagnostics must stay consistent without invalidate (heartbeat hot path).
        let diag2 = service.diagnostics();
        assert_eq!(diag.get("cpu_deps_ready"), diag2.get("cpu_deps_ready"));
        assert_eq!(diag.get("cuda_deps_ready"), diag2.get("cuda_deps_ready"));
    }

    #[test]
    fn select_model_updates_config_variant_and_catalog_active_flag() {
        let dir = tempfile::tempdir().unwrap();
        let service = LocalAsrModuleService::new(dir.path(), dir.path().join("logs"));
        let int8_dir = model_manager::model_dir_for_variant(service.module_dir(), MODEL_VARIANT_INT8);
        std::fs::create_dir_all(&int8_dir).unwrap();
        for name in ModelVariant::Int8.required_files() {
            let min = if name.ends_with(".onnx.data") {
                1_000_000usize
            } else if name.ends_with(".onnx") {
                1_000
            } else {
                32
            };
            std::fs::write(int8_dir.join(name), vec![b'x'; min]).unwrap();
        }

        let status = service.select_model("parakeet_tdt", MODEL_VARIANT_FP32).expect("select fp32");
        assert_eq!(status.active_model_variant, MODEL_VARIANT_FP32);
        assert!(status.models.iter().any(|entry| {
            entry.family == "parakeet_tdt" && entry.variant == MODEL_VARIANT_FP32 && entry.active
        }));

        let status = service.select_model("parakeet_tdt", MODEL_VARIANT_INT8).expect("select int8");
        assert_eq!(status.active_model_variant, MODEL_VARIANT_INT8);
        let config = service.load_config().expect("load config");
        assert_eq!(config.model.variant, MODEL_VARIANT_INT8);
        assert!(!config.model.path.is_empty());
        assert_eq!(
            status.models.iter().filter(|entry| entry.family == "parakeet_tdt").count(),
            3
        );
    }
}
