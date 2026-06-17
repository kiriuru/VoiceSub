use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::Value;

use crate::compact_log_line::{format_structured_runtime_line, should_write_runtime_event};
use crate::diagnostics::is_runtime_events_verbose_enabled;
use crate::log_rotation::{DEFAULT_BACKUP_COUNT, DEFAULT_MAX_BYTES, rotate_if_needed};
use crate::redaction::redact_mapping;
use crate::structured_log_compact::compact_mapping_for_runtime_log;

const LOG_FILE: &str = "runtime-events.log";
const MAX_BYTES: u64 = DEFAULT_MAX_BYTES;
const BACKUP_COUNT: u32 = DEFAULT_BACKUP_COUNT;

#[derive(Debug)]
pub struct StructuredRuntimeLogger {
    logs_dir: PathBuf,
    lock: Mutex<()>,
}

impl StructuredRuntimeLogger {
    pub fn new(logs_dir: impl AsRef<Path>) -> Self {
        let logger = Self {
            logs_dir: logs_dir.as_ref().to_path_buf(),
            lock: Mutex::new(()),
        };
        logger.reset();
        logger
    }

    pub fn reset(&self) {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let _ = fs::create_dir_all(&self.logs_dir);
        let path = self.log_path();
        let _ = fs::write(&path, "");
        for index in 1..=BACKUP_COUNT {
            let rotated = path.with_extension(format!("log.{index}"));
            let _ = fs::remove_file(rotated);
        }
    }

    pub fn log_path(&self) -> PathBuf {
        self.logs_dir.join(LOG_FILE)
    }

    pub fn log(
        &self,
        channel: &str,
        event: &str,
        source: Option<&str>,
        payload: Option<BTreeMap<String, Value>>,
    ) {
        let normalized_event = event.trim();
        if normalized_event.is_empty() {
            return;
        }
        if !should_write_runtime_event(normalized_event, is_runtime_events_verbose_enabled()) {
            return;
        }

        let mut fields = BTreeMap::new();
        if let Some(map) = payload {
            let redacted = redact_mapping(&map);
            fields.extend(compact_mapping_for_runtime_log(&redacted));
        }

        let line = format_structured_runtime_line(normalized_event, channel, source, &fields);
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let _ = self.write_line_locked(&line);
    }

    fn write_line_locked(&self, line: &str) -> bool {
        let _ = fs::create_dir_all(&self.logs_dir);
        let path = self.log_path();
        self.rotate_if_needed_locked(&path);
        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut file| writeln!(file, "{line}"))
        {
            Ok(()) => true,
            Err(_) => false,
        }
    }

    fn rotate_if_needed_locked(&self, path: &Path) {
        rotate_if_needed(path, MAX_BYTES, BACKUP_COUNT);
    }
}

pub fn runtime_trace(
    logger: Option<&StructuredRuntimeLogger>,
    event: &str,
    source: &str,
    fields: BTreeMap<String, Value>,
) {
    let Some(logger) = logger else {
        return;
    };
    logger.log("runtime_metrics", event, Some(source), Some(fields));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::set_config_full_logging_enabled;
    use crate::env_test;

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("vs-srl-{name}-{}", std::process::id()))
    }

    #[test]
    fn reset_truncates_existing_log() {
        let dir = temp_dir("reset");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join(LOG_FILE);
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "old line\n").unwrap();
        let _logger = StructuredRuntimeLogger::new(&dir);
        assert_eq!(fs::read_to_string(&path).unwrap(), "");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn default_skips_dbg_and_vrb_events() {
        let _guard = env_test::lock();
        env_test::clear_env_vars(env_test::RUNTIME_VERBOSE_KEYS);
        env_test::reset_compact_logging_env();
        let dir = temp_dir("gate");
        let _ = fs::remove_dir_all(&dir);

        let logger = StructuredRuntimeLogger::new(&dir);
        logger.log(
            "translation_dispatcher",
            "translation_queue_depth_changed",
            Some("translation_dispatcher"),
            Some(BTreeMap::from([("queue_depth".into(), Value::from(0))])),
        );
        logger.log(
            "translation_dispatcher",
            "translation_publish_accepted",
            Some("translation_dispatcher"),
            Some(BTreeMap::from([("sequence".into(), Value::from(1))])),
        );
        logger.log(
            "browser_recognition",
            "browser_degraded",
            Some("browser_asr_gateway"),
            Some(BTreeMap::from([(
                "reason".into(),
                Value::String("noise".into()),
            )])),
        );
        logger.log(
            "runtime_state",
            "runtime_status_broadcast",
            Some("runtime_state_controller"),
            Some(BTreeMap::from([
                ("heartbeat".into(), Value::from(true)),
                ("important_change".into(), Value::from(false)),
            ])),
        );
        logger.log(
            "subtitle_router",
            "subtitle_payload_published",
            Some("subtitle_router"),
            Some(BTreeMap::from([("sequence".into(), Value::from(1))])),
        );
        logger.log(
            "runtime_ingest",
            "asr_ingest_partial_published",
            Some("runtime_ingest"),
            Some(BTreeMap::from([("sequence".into(), Value::from(1))])),
        );

        let joined = fs::read_to_string(logger.log_path()).unwrap_or_default();
        assert!(!joined.contains("translation_queue_depth_changed"));
        assert!(!joined.contains("runtime_status_broadcast"));
        assert!(!joined.contains("subtitle_payload_published"));
        assert!(!joined.contains("asr_ingest_partial_published"));
        assert!(joined.contains("translation_publish_accepted"));
        assert!(joined.contains("browser_degraded"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn verbose_mode_writes_dbg_events() {
        let _guard = env_test::lock();
        env_test::clear_env_vars(env_test::RUNTIME_VERBOSE_KEYS);
        env_test::reset_compact_logging_env();
        let dir = temp_dir("verbose");
        let _ = fs::remove_dir_all(&dir);
        set_config_full_logging_enabled(true);

        let logger = StructuredRuntimeLogger::new(&dir);
        logger.log(
            "translation_dispatcher",
            "translation_job_started",
            Some("translation_dispatcher"),
            Some(BTreeMap::from([("sequence".into(), Value::from(9))])),
        );

        let joined = fs::read_to_string(logger.log_path()).unwrap_or_default();
        assert!(joined.contains("translation_job_started"));
        set_config_full_logging_enabled(false);
        let _ = fs::remove_dir_all(dir);
    }
}
