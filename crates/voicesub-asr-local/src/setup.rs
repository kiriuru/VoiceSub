//! One-time module setup checklist — persisted in module config.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{EXECUTION_PROVIDER_CUDA, LocalAsrConfig, LocalAsrSetupConfig};
use crate::deps::LocalAsrEnvCheck;
use crate::status::model_is_installed;
use crate::test_session::{TestBenchPhase, TestBenchSnapshot};

pub fn deps_ready_for_provider(env: &LocalAsrEnvCheck, provider: &str) -> bool {
    if provider == EXECUTION_PROVIDER_CUDA {
        env.cuda_deps_ready
    } else {
        env.cpu_deps_ready
    }
}

pub fn test_received_parakeet_final(snap: &TestBenchSnapshot) -> bool {
    snap.finalized_segments > 0 || !snap.transcript.trim().is_empty()
}

pub fn clear_setup(config: &mut LocalAsrConfig) {
    config.setup = LocalAsrSetupConfig::default();
}

pub fn setup_is_valid(config: &LocalAsrConfig, provider: &str) -> bool {
    config.setup.setup_complete
        && config.setup.validated_execution_provider == provider
        && config.setup.mic_test_passed
        && config.setup.parakeet_final_received
}

/// Record mic test / Parakeet final progress from a completed test bench run.
pub fn apply_test_bench_progress(config: &mut LocalAsrConfig, snap: &TestBenchSnapshot) {
    if snap.phase != TestBenchPhase::Done {
        return;
    }
    config.setup.mic_test_passed = true;
    if test_received_parakeet_final(snap) {
        config.setup.parakeet_final_received = true;
    }
}

/// Returns `true` when setup just became complete.
pub fn try_finalize_setup(
    config: &mut LocalAsrConfig,
    env: &LocalAsrEnvCheck,
    module_dir: &Path,
) -> bool {
    let provider = config.inference.execution_provider.clone();
    if !deps_ready_for_provider(env, &provider) || !model_is_installed(config, module_dir) {
        return false;
    }
    if !config.setup.mic_test_passed || !config.setup.parakeet_final_received {
        return false;
    }
    if setup_is_valid(config, &provider) {
        return false;
    }
    config.setup.setup_complete = true;
    config.setup.validated_execution_provider = provider;
    if config.setup.completed_at.is_empty() {
        config.setup.completed_at = now_unix_seconds();
    }
    true
}

fn now_unix_seconds() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LocalAsrConfig;
    use crate::test_session::TestBenchSnapshot;

    #[test]
    fn parakeet_final_detects_transcript_or_segments() {
        let mut snap = TestBenchSnapshot::default();
        assert!(!test_received_parakeet_final(&snap));
        snap.transcript = "hello".into();
        assert!(test_received_parakeet_final(&snap));
    }

    #[test]
    fn apply_test_progress_marks_mic_and_final() {
        let mut config = LocalAsrConfig::default();
        let snap = TestBenchSnapshot {
            phase: TestBenchPhase::Done,
            transcript: "test".into(),
            finalized_segments: 1,
            ..TestBenchSnapshot::default()
        };
        apply_test_bench_progress(&mut config, &snap);
        assert!(config.setup.mic_test_passed);
        assert!(config.setup.parakeet_final_received);
    }
}
