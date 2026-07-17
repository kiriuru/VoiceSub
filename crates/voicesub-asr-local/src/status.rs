use std::path::Path;

use serde::Serialize;

use crate::config::{EXECUTION_PROVIDER_CPU, EXECUTION_PROVIDER_CUDA, LocalAsrConfig};
use crate::deps::LocalAsrEnvCheck;
use crate::inference::InferenceSnapshot;
use crate::model_family::ModelFamily;
use crate::model_manager::{build_all_model_catalogs, is_model_installed_for, resolve_model_dir};
use crate::setup::{deps_ready_for_provider, setup_is_valid};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalAsrModulePhase {
    Unconfigured,
    DepsMissing,
    DepsReady,
    ModelMissing,
    ModelReady,
    Ready,
    Running,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalAsrSetupChecklist {
    pub deps_ready: bool,
    pub model_installed: bool,
    pub mic_test_passed: bool,
    pub parakeet_final_received: bool,
    pub setup_complete: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalAsrModuleStatus {
    pub phase: LocalAsrModulePhase,
    pub ready: bool,
    pub cuda_ready: bool,
    pub deps_ready: bool,
    pub execution_provider: String,
    pub active_execution_provider: String,
    pub message: String,
    pub last_error: Option<String>,
    pub env: LocalAsrEnvCheck,
    pub model_installed: bool,
    pub model_loaded: bool,
    pub model_load_ms: Option<u64>,
    pub probe_cpu_ok: Option<bool>,
    pub probe_cuda_ok: Option<bool>,
    pub ort_profiling_active: bool,
    pub ort_profiling_decode_count: u32,
    pub ort_profiling_max_decodes: u32,
    pub ort_profiling_stopped_budget: bool,
    pub last_ort_profile_path: Option<String>,
    pub last_decode_timing: Option<crate::decode_timing::DecodeTimingBreakdown>,
    pub active_model_family: String,
    pub active_model_variant: String,
    pub models: Vec<crate::model_manager::ModelCatalogEntry>,
    pub setup: LocalAsrSetupChecklist,
}

pub fn build_status(
    config: &LocalAsrConfig,
    env: LocalAsrEnvCheck,
    module_dir: &Path,
    inference: &InferenceSnapshot,
) -> LocalAsrModuleStatus {
    let provider = config.inference.execution_provider.clone();
    let model_installed = model_is_installed(config, module_dir);
    let deps_ready = deps_ready_for_provider(&env, &provider);
    let setup_complete = setup_is_valid(config, &provider);
    let active_execution_provider = if inference.model_loaded {
        inference.active_execution_provider.clone()
    } else {
        provider.clone()
    };

    let setup = LocalAsrSetupChecklist {
        deps_ready,
        model_installed,
        mic_test_passed: config.setup.mic_test_passed,
        parakeet_final_received: config.setup.parakeet_final_received,
        setup_complete,
    };

    let (phase, message, last_error) = if !env.vcruntime.ok {
        (
            LocalAsrModulePhase::DepsMissing,
            "Install VC++ runtime (or use a system with Visual C++ 2015–2022 redistributable)".into(),
            Some(format!("missing: {}", env.vcruntime.missing.join(", "))),
        )
    } else if !deps_ready {
        let (message, last_error) = if provider == EXECUTION_PROVIDER_CUDA {
            (
                "Complete GPU dependencies (ORT GPU, CUDA redist, NVIDIA driver)".into(),
                deps_missing_detail(&env, EXECUTION_PROVIDER_CUDA),
            )
        } else {
            (
                "Download ONNX Runtime (CPU or GPU)".into(),
                deps_missing_detail(&env, EXECUTION_PROVIDER_CPU),
            )
        };
        (LocalAsrModulePhase::DepsMissing, message, last_error)
    } else if !model_installed {
        (
            LocalAsrModulePhase::ModelMissing,
            "Download the ASR model in the Local ASR module".into(),
            None,
        )
    } else if !setup_complete {
        (
            LocalAsrModulePhase::ModelReady,
            setup_pending_message(&setup, inference.model_loaded),
            inference.last_error.clone(),
        )
    } else {
        (
            LocalAsrModulePhase::Ready,
            if inference.model_loaded {
                format!("Local ASR ready ({active_execution_provider} EP, model loaded)")
            } else {
                format!(
                    "Local ASR configured ({active_execution_provider} EP) — model loads on Live Start"
                )
            },
            inference.last_error.clone(),
        )
    };

    let ready = setup_complete && deps_ready && model_installed;
    let cuda_ready = env.cuda_deps_ready
        && (inference.probe_cuda_ok == Some(true)
            || (setup_complete && config.setup.validated_execution_provider == EXECUTION_PROVIDER_CUDA));

    LocalAsrModuleStatus {
        phase,
        ready,
        cuda_ready,
        deps_ready,
        execution_provider: provider,
        active_execution_provider,
        message,
        last_error,
        env,
        model_installed,
        model_loaded: inference.model_loaded,
        model_load_ms: inference.model_load_ms,
        probe_cpu_ok: inference.probe_cpu_ok,
        probe_cuda_ok: inference.probe_cuda_ok,
        ort_profiling_active: inference.ort_profiling_active,
        ort_profiling_decode_count: inference.ort_profiling_decode_count,
        ort_profiling_max_decodes: inference.ort_profiling_max_decodes,
        ort_profiling_stopped_budget: inference.ort_profiling_stopped_budget,
        last_ort_profile_path: inference.last_ort_profile_path.clone(),
        last_decode_timing: inference.last_decode_timing.clone(),
        active_model_family: config.model.family.clone(),
        active_model_variant: config.model.variant.clone(),
        models: build_all_model_catalogs(
            module_dir,
            &config.model.family,
            &config.model.variant,
        ),
        setup,
    }
}

fn deps_missing_detail(env: &LocalAsrEnvCheck, provider: &str) -> Option<String> {
    let mut missing = Vec::new();
    if !env.vcruntime.ok {
        missing.extend(env.vcruntime.missing.iter().cloned());
    }
    if provider == EXECUTION_PROVIDER_CUDA {
        if !env.ort_gpu.ok {
            missing.extend(env.ort_gpu.missing.iter().cloned());
        }
        if !env.cuda_redist.ok {
            missing.extend(env.cuda_redist.missing.iter().cloned());
        }
        if !env.cuda_toolkit.ok {
            if let Some(message) = env.cuda_toolkit.message.as_ref() {
                missing.push(message.clone());
            }
        }
    } else if !env.ort_cpu.ok && !env.ort_gpu.ok {
        missing.extend(env.ort_cpu.missing.iter().cloned());
    }
    if missing.is_empty() {
        None
    } else {
        Some(format!("missing: {}", missing.join(", ")))
    }
}

fn setup_pending_message(setup: &LocalAsrSetupChecklist, model_loaded: bool) -> String {
    if !setup.deps_ready {
        return "Complete dependencies for the selected execution provider".into();
    }
    if !setup.model_installed {
        return "Download the ASR model".into();
    }
    if !model_loaded {
        return "Warm-load the model, run the mic test, and receive a final transcript".into();
    }
    if !setup.mic_test_passed {
        return "Run the mic test bench to finish one-time setup".into();
    }
    if !setup.parakeet_final_received {
        return "Speak during the mic test until Parakeet returns a final line".into();
    }
    "Complete the setup checklist".into()
}

pub(crate) fn model_is_installed(config: &LocalAsrConfig, module_dir: &Path) -> bool {
    let family = ModelFamily::parse(&config.model.family).unwrap_or(ModelFamily::ParakeetTdt);
    let model_dir = resolve_model_dir(
        &config.model.path,
        &config.model.family,
        &config.model.variant,
        module_dir,
    );
    is_model_installed_for(&model_dir, family, &config.model.variant)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LocalAsrConfig;
    use crate::deps::env_check;
    use crate::inference::InferenceSnapshot;

    #[test]
    fn deps_missing_detail_lists_gpu_stack_gaps() {
        let dir = tempfile::tempdir().unwrap();
        let env = env_check(dir.path());
        let detail = deps_missing_detail(&env, EXECUTION_PROVIDER_CUDA).expect("detail");
        assert!(detail.contains("providers_cuda") || detail.contains("CUDA Toolkit"));
    }

    #[test]
    fn deps_missing_when_no_ort() {
        let dir = tempfile::tempdir().unwrap();
        let config = LocalAsrConfig::default();
        let status = build_status(
            &config,
            env_check(dir.path()),
            dir.path(),
            &InferenceSnapshot::default(),
        );
        assert_eq!(status.phase, LocalAsrModulePhase::DepsMissing);
        assert!(!status.ready);
    }

    #[test]
    fn setup_pending_when_model_on_disk_but_checklist_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        let config = LocalAsrConfig::default();
        let status = build_status(
            &config,
            env_check(dir.path()),
            dir.path(),
            &InferenceSnapshot::default(),
        );
        assert!(!status.setup.setup_complete);
        assert!(!status.ready);
    }
}
