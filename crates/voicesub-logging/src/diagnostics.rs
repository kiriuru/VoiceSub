//! Environment- and config-driven gates for deep diagnostic JSONL traces (SST-compatible).

use std::sync::atomic::{AtomicBool, Ordering};

const TRUE_TOKENS: &[&str] = &["1", "true", "yes", "on"];
const FALSE_TOKENS: &[&str] = &["0", "false", "no", "off"];

static CONFIG_FULL_LOGGING_ENABLED: AtomicBool = AtomicBool::new(false);

fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|value| value.trim().to_ascii_lowercase())
}

fn is_truthy(name: &str) -> bool {
    env_value(name)
        .as_deref()
        .is_some_and(|value| TRUE_TOKENS.contains(&value))
}

fn is_falsy(name: &str) -> bool {
    env_value(name)
        .as_deref()
        .is_some_and(|value| FALSE_TOKENS.contains(&value))
}

pub fn set_config_full_logging_enabled(enabled: bool) {
    CONFIG_FULL_LOGGING_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_config_full_logging_enabled() -> bool {
    CONFIG_FULL_LOGGING_ENABLED.load(Ordering::Relaxed)
}

/// Master switch — config ``logging.full_enabled`` or ``VOICESUB_DEEP_DIAGNOSTICS`` / ``SST_DEEP_DIAGNOSTICS``.
pub fn is_deep_diagnostics_enabled() -> bool {
    is_config_full_logging_enabled()
        || is_truthy("VOICESUB_DEEP_DIAGNOSTICS")
        || is_truthy("SST_DEEP_DIAGNOSTICS")
}

/// Subtitle FSM / TTL / overlay contract trace (`logs/subtitle-trace.jsonl`).
pub fn is_subtitle_trace_enabled() -> bool {
    if is_falsy("VOICESUB_TRACE_SUBTITLE") || is_falsy("SST_TRACE_SUBTITLE") {
        return false;
    }
    is_deep_diagnostics_enabled()
        || is_truthy("VOICESUB_TRACE_SUBTITLE")
        || is_truthy("SST_TRACE_SUBTITLE")
}

/// TTS module trace (`logs/tts-trace.jsonl`).
pub fn is_tts_trace_enabled() -> bool {
    if is_falsy("VOICESUB_TRACE_TTS") || is_falsy("SST_TRACE_TTS") {
        return false;
    }
    is_deep_diagnostics_enabled()
        || is_truthy("VOICESUB_TRACE_TTS")
        || is_truthy("SST_TRACE_TTS")
}

/// Browser ASR gateway trace (`logs/browser-trace.jsonl`).
pub fn is_browser_trace_enabled() -> bool {
    if is_falsy("VOICESUB_TRACE_BROWSER") || is_falsy("SST_TRACE_BROWSER") {
        return false;
    }
    is_deep_diagnostics_enabled()
        || is_truthy("VOICESUB_TRACE_BROWSER")
        || is_truthy("SST_TRACE_BROWSER")
}

/// OBS closed captions trace (`logs/obs-trace.jsonl`).
pub fn is_obs_trace_enabled() -> bool {
    if is_falsy("VOICESUB_TRACE_OBS") || is_falsy("SST_TRACE_OBS") {
        return false;
    }
    is_deep_diagnostics_enabled()
        || is_truthy("VOICESUB_TRACE_OBS")
        || is_truthy("SST_TRACE_OBS")
}

/// Dashboard / overlay render trace (`logs/ui-trace.jsonl`).
pub fn is_ui_trace_enabled() -> bool {
    if is_falsy("VOICESUB_TRACE_UI") || is_falsy("SST_TRACE_UI") {
        return false;
    }
    is_deep_diagnostics_enabled()
        || is_truthy("VOICESUB_TRACE_UI")
        || is_truthy("SST_TRACE_UI")
}

/// WebSocket hub trace (`logs/ws-trace.jsonl`).
pub fn is_ws_trace_enabled() -> bool {
    if is_falsy("VOICESUB_TRACE_WS") || is_falsy("SST_TRACE_WS") {
        return false;
    }
    is_deep_diagnostics_enabled()
        || is_truthy("VOICESUB_TRACE_WS")
        || is_truthy("SST_TRACE_WS")
}

/// Runtime lifecycle / ingest pipeline trace (`logs/pipeline-trace.jsonl`).
pub fn is_pipeline_trace_enabled() -> bool {
    if is_falsy("VOICESUB_TRACE_PIPELINE") || is_falsy("SST_TRACE_PIPELINE") {
        return false;
    }
    is_deep_diagnostics_enabled()
        || is_truthy("VOICESUB_TRACE_PIPELINE")
        || is_truthy("SST_TRACE_PIPELINE")
}

/// Gate high-frequency DBG/VRB rows in ``logs/runtime-events.log`` (SST compact default).
pub fn is_runtime_events_verbose_enabled() -> bool {
    is_deep_diagnostics_enabled()
        || is_truthy("VOICESUB_TRACE_RUNTIME_EVENTS_VERBOSE")
        || is_truthy("SST_TRACE_RUNTIME_EVENTS_VERBOSE")
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    use super::*;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_test_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn clear(keys: &[&str]) {
        for key in keys {
            std::env::remove_var(key);
        }
    }

    fn reset_config_flag() {
        set_config_full_logging_enabled(false);
    }

    #[test]
    fn subtitle_trace_off_in_compact_mode() {
        let _guard = env_test_lock();
        reset_config_flag();
        clear(&[
            "VOICESUB_TRACE_SUBTITLE",
            "SST_TRACE_SUBTITLE",
            "VOICESUB_DEEP_DIAGNOSTICS",
            "SST_DEEP_DIAGNOSTICS",
        ]);
        assert!(!is_subtitle_trace_enabled());
    }

    #[test]
    fn subtitle_trace_on_when_config_full_logging_enabled() {
        let _guard = env_test_lock();
        reset_config_flag();
        clear(&["VOICESUB_TRACE_SUBTITLE", "VOICESUB_DEEP_DIAGNOSTICS"]);
        set_config_full_logging_enabled(true);
        assert!(is_subtitle_trace_enabled());
        reset_config_flag();
    }

    #[test]
    fn subtitle_trace_can_be_enabled_via_env() {
        let _guard = env_test_lock();
        reset_config_flag();
        clear(&["VOICESUB_TRACE_SUBTITLE", "VOICESUB_DEEP_DIAGNOSTICS"]);
        std::env::set_var("VOICESUB_TRACE_SUBTITLE", "1");
        assert!(is_subtitle_trace_enabled());
        clear(&["VOICESUB_TRACE_SUBTITLE"]);
    }

    #[test]
    fn ws_trace_follows_full_logging() {
        let _guard = env_test_lock();
        reset_config_flag();
        clear(&["VOICESUB_TRACE_WS", "VOICESUB_DEEP_DIAGNOSTICS"]);
        assert!(!is_ws_trace_enabled());
        set_config_full_logging_enabled(true);
        assert!(is_ws_trace_enabled());
        reset_config_flag();
    }

    #[test]
    fn pipeline_trace_can_be_enabled_via_env() {
        let _guard = env_test_lock();
        reset_config_flag();
        clear(&["VOICESUB_TRACE_PIPELINE", "VOICESUB_DEEP_DIAGNOSTICS"]);
        std::env::set_var("VOICESUB_TRACE_PIPELINE", "1");
        assert!(is_pipeline_trace_enabled());
        clear(&["VOICESUB_TRACE_PIPELINE"]);
    }

    #[test]
    fn runtime_events_verbose_follows_full_logging() {
        let _guard = env_test_lock();
        reset_config_flag();
        clear(&["VOICESUB_TRACE_RUNTIME_EVENTS_VERBOSE", "VOICESUB_DEEP_DIAGNOSTICS"]);
        assert!(!is_runtime_events_verbose_enabled());
        set_config_full_logging_enabled(true);
        assert!(is_runtime_events_verbose_enabled());
        reset_config_flag();
    }
}
