//! Tracing setup and log directory helpers.

mod compact_log_line;
mod diagnostics;
mod jsonl_trace;
mod lifecycle;
mod log_rotation;
mod redaction;
mod rotating_log_file;
mod session;
mod structured_log_compact;
mod structured_runtime_logger;
mod browser_trace;
mod obs_trace;
mod pipeline_trace;
mod subtitle_trace;
mod tts_trace;
mod ui_trace;
mod ws_trace;

pub use diagnostics::{
    is_config_full_logging_enabled, is_deep_diagnostics_enabled, is_runtime_events_verbose_enabled,
    is_browser_trace_enabled, is_obs_trace_enabled, is_pipeline_trace_enabled,
    is_subtitle_trace_enabled, is_tts_trace_enabled, is_ui_trace_enabled, is_ws_trace_enabled,
    set_config_full_logging_enabled, should_persist_client_log,
};
pub use lifecycle::{
    complete_graceful_shutdown, install_lifecycle_hooks, log_shutdown_begin, log_shutdown_step,
    read_session_lifecycle_record, session_lifecycle_path, SessionExitState, SessionLifecycleRecord,
};

pub use redaction::{redact_data, redact_text, REDACTED_VALUE};
pub use compact_log_line::{should_write_runtime_event, structured_event_level};
pub use session::{ClientLogResult, SessionLogDiagnostics, SessionLogManager};
pub use structured_runtime_logger::{runtime_trace, StructuredRuntimeLogger};
pub use browser_trace::{browser_trace, configure_browser_trace_log};
pub use obs_trace::{configure_obs_trace_log, obs_trace};
pub use pipeline_trace::{configure_pipeline_trace_log, pipeline_trace};
pub use ws_trace::{configure_ws_trace_log, ws_trace};
pub use subtitle_trace::{configure_subtitle_trace_log, subtitle_trace, subtitle_trace_mapping};
pub use tts_trace::{configure_tts_trace_log, tts_trace};
pub use ui_trace::{configure_ui_trace_log, ui_trace, ui_trace_mapping};

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::rotating_log_file::default_core_log_writer;

/// Initialize global tracing to stderr only (tests / early bootstrap).
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true))
        .try_init();
}

/// Initialize tracing with backbone log file (`logs/core.log`) + stderr.
pub fn init_tracing_backbone(project_root: &Path) {
    let _ = ensure_logs_dir(project_root);
    let logs = logs_dir(project_root);
    configure_subtitle_trace_log(&logs);
    configure_browser_trace_log(&logs);
    configure_obs_trace_log(&logs);
    configure_tts_trace_log(&logs);
    configure_ui_trace_log(&logs);
    configure_ws_trace_log(&logs);
    configure_pipeline_trace_log(&logs);
    let paths = backbone_log_paths(project_root);
    rotate_to_old_log(&paths.core);
    rotate_to_old_log(&paths.runtime_events);

    let core_writer = default_core_log_writer(&paths.core);
    let default_filter = if is_deep_diagnostics_enabled() {
        let mut parts = vec!["info".to_string()];
        if is_subtitle_trace_enabled() {
            parts.push("voicesub_subtitle=debug".into());
            parts.push("voicesub_runtime=debug".into());
        }
        if is_tts_trace_enabled() {
            parts.push("voicesub_tts=debug".into());
            parts.push("voicesub_audio=debug".into());
        }
        if is_browser_trace_enabled() {
            parts.push("voicesub_browser=debug".into());
        }
        if is_obs_trace_enabled() {
            parts.push("voicesub_obs=debug".into());
        }
        if is_ws_trace_enabled() {
            parts.push("voicesub_ws=debug".into());
        }
        if is_pipeline_trace_enabled() {
            parts.push("voicesub_pipeline=debug".into());
        }
        parts.push("voicesub_translation=debug".into());
        parts.push("voicesub_http=info".into());
        parts.join(",")
    } else {
        "warn".to_string()
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true))
        .with(fmt::layer().with_writer(core_writer).with_ansi(false))
        .try_init();
}

pub fn logs_dir(project_root: &Path) -> PathBuf {
    project_root.join("logs")
}

pub fn backbone_log_paths(project_root: &Path) -> BackboneLogs {
    let dir = logs_dir(project_root);
    BackboneLogs {
        core: dir.join("core.log"),
        runtime_events: dir.join("runtime-events.log"),
        session_latest: dir.join("session-latest.jsonl"),
    }
}

/// Opt-in JSONL trace files written when deep diagnostics are enabled.
pub const DEEP_TRACE_LOG_FILES: &[&str] = &[
    "subtitle-trace.jsonl",
    "tts-trace.jsonl",
    "browser-trace.jsonl",
    "obs-trace.jsonl",
    "ui-trace.jsonl",
    "ws-trace.jsonl",
    "pipeline-trace.jsonl",
];

pub fn deep_trace_log_paths(logs_dir: &Path) -> Vec<PathBuf> {
    DEEP_TRACE_LOG_FILES
        .iter()
        .map(|name| logs_dir.join(name))
        .collect()
}

#[derive(Debug, Clone)]
pub struct BackboneLogs {
    pub core: PathBuf,
    pub runtime_events: PathBuf,
    pub session_latest: PathBuf,
}

pub fn ensure_logs_dir(project_root: &Path) -> io::Result<PathBuf> {
    let dir = logs_dir(project_root);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Apply ``logging.full_enabled`` at runtime (trace JSONL gates + compact runtime-events verbosity).
pub fn apply_logging_preferences(logs_dir: &Path, full_enabled: bool) {
    set_config_full_logging_enabled(full_enabled);
    configure_subtitle_trace_log(logs_dir);
    configure_browser_trace_log(logs_dir);
    configure_obs_trace_log(logs_dir);
    configure_tts_trace_log(logs_dir);
    configure_ui_trace_log(logs_dir);
    configure_ws_trace_log(logs_dir);
    configure_pipeline_trace_log(logs_dir);
}

fn rotate_to_old_log(path: &Path) {
    if !path.is_file() {
        return;
    }
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    let backup = parent.join(format!("{stem}.old.log"));
    let _ = fs::remove_file(&backup);
    let _ = fs::rename(path, backup);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backbone_paths_under_logs() {
        let root = Path::new("F:/AI/VoiceSub");
        let paths = backbone_log_paths(root);
        assert!(paths.core.ends_with("logs/core.log"));
        assert!(paths.runtime_events.ends_with("logs/runtime-events.log"));
        assert!(paths.session_latest.ends_with("logs/session-latest.jsonl"));
    }

    #[test]
    fn rotate_renames_core_log() {
        let dir = std::env::temp_dir().join(format!("voicesub-log-rotate-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let core = dir.join("core.log");
        fs::write(&core, "line\n").unwrap();
        rotate_to_old_log(&core);
        assert!(!core.is_file());
        assert!(dir.join("core.old.log").is_file());
        let _ = fs::remove_dir_all(dir);
    }
}
