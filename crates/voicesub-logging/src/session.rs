use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::diagnostics::should_persist_client_log;
use crate::redaction::{redact_mapping, redact_text};

const LOG_FILE: &str = "session-latest.jsonl";
const MAX_LINES: usize = 5000;
const CHANNELS: &[&str] = &["dashboard", "overlay", "browser_worker"];

#[derive(Debug, Clone, Default, Serialize)]
pub struct SessionLogDiagnostics {
    pub client_log_events_received: u64,
    pub client_log_events_written: u64,
    pub client_log_events_dropped: u64,
    pub client_log_last_error: Option<String>,
    pub client_log_last_error_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientLogResult {
    pub ok: bool,
    pub logged: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

pub struct SessionLogManager {
    logs_dir: PathBuf,
    lock: Mutex<()>,
    diagnostics: Mutex<SessionLogDiagnostics>,
}

impl SessionLogManager {
    pub fn new(logs_dir: impl AsRef<Path>) -> Self {
        let manager = Self {
            logs_dir: logs_dir.as_ref().to_path_buf(),
            lock: Mutex::new(()),
            diagnostics: Mutex::new(SessionLogDiagnostics::default()),
        };
        manager.reset();
        manager
    }

    pub fn reset(&self) {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let _ = fs::create_dir_all(&self.logs_dir);
        let _ = self.safe_write_text_locked("");
        if let Ok(mut diag) = self.diagnostics.lock() {
            *diag = SessionLogDiagnostics::default();
        }
    }

    pub fn diagnostics(&self) -> SessionLogDiagnostics {
        self.diagnostics
            .lock()
            .map(|d| d.clone())
            .unwrap_or_default()
    }

    pub fn log_path(&self) -> PathBuf {
        self.logs_dir.join(LOG_FILE)
    }

    pub fn log(
        &self,
        channel: &str,
        message: &str,
        source: Option<&str>,
        details: Option<BTreeMap<String, Value>>,
    ) -> ClientLogResult {
        if let Ok(mut diag) = self.diagnostics.lock() {
            diag.client_log_events_received += 1;
        }

        let normalized_channel = Self::normalize_channel(channel);
        let normalized_message: String = redact_text(message)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        if normalized_message.is_empty() {
            if let Ok(mut diag) = self.diagnostics.lock() {
                diag.client_log_events_dropped += 1;
            }
            return ClientLogResult {
                ok: true,
                logged: false,
                reason: Some("empty_message".into()),
            };
        }

        if !should_persist_client_log(channel, source) {
            if let Ok(mut diag) = self.diagnostics.lock() {
                diag.client_log_events_dropped += 1;
            }
            return ClientLogResult {
                ok: true,
                logged: false,
                reason: Some("compact_mode_filtered".into()),
            };
        }

        let sanitized_details = details.map(|map| redact_mapping(&map));
        let record = Self::format_record(
            &normalized_channel,
            &normalized_message,
            source,
            sanitized_details,
        );

        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        if !self.append_record_locked(&record) {
            self.mark_drop_locked("log_write_failed");
            return ClientLogResult {
                ok: true,
                logged: false,
                reason: Some("log_write_failed".into()),
            };
        }
        if let Ok(mut diag) = self.diagnostics.lock() {
            diag.client_log_events_written += 1;
        }
        ClientLogResult {
            ok: true,
            logged: true,
            reason: None,
        }
    }

    fn normalize_channel(channel: &str) -> String {
        let normalized = channel.trim().to_ascii_lowercase();
        if CHANNELS.contains(&normalized.as_str()) {
            normalized
        } else {
            "dashboard".into()
        }
    }

    fn format_record(
        channel: &str,
        message: &str,
        source: Option<&str>,
        details: Option<BTreeMap<String, Value>>,
    ) -> BTreeMap<String, Value> {
        let mut record = BTreeMap::new();
        record.insert("timestamp_utc".into(), Value::String(utc_now_iso()));
        record.insert("channel".into(), Value::String(channel.into()));
        record.insert("type".into(), Value::String("event".into()));
        let source_value = source.unwrap_or("").trim().to_ascii_lowercase();
        if source_value.is_empty() {
            record.insert("source".into(), Value::Null);
        } else {
            record.insert("source".into(), Value::String(source_value));
        }
        record.insert("message".into(), Value::String(message.into()));
        if let Some(map) = details.filter(|m| !m.is_empty()) {
            record.insert("details".into(), Value::Object(map.into_iter().collect()));
        } else {
            record.insert("details".into(), Value::Null);
        }
        record
    }

    fn append_record_locked(&self, record: &BTreeMap<String, Value>) -> bool {
        let line = match serde_json::to_string(record) {
            Ok(json) => format!("{json}\n"),
            Err(_) => return false,
        };
        if !self.safe_append_line_locked(&line) {
            return false;
        }
        self.truncate_to_max_lines_locked();
        true
    }

    fn safe_write_text_locked(&self, text: &str) -> bool {
        let _ = fs::create_dir_all(&self.logs_dir);
        for attempt in 0..2 {
            match fs::write(self.log_path(), text) {
                Ok(()) => return true,
                Err(err) => {
                    self.remember_error_locked(&err.to_string(), "io");
                    if attempt == 0 {
                        thread::sleep(Duration::from_millis(20));
                    }
                }
            }
        }
        false
    }

    fn safe_append_line_locked(&self, line: &str) -> bool {
        let _ = fs::create_dir_all(&self.logs_dir);
        for attempt in 0..2 {
            match OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.log_path())
                .and_then(|mut file| file.write_all(line.as_bytes()))
            {
                Ok(()) => return true,
                Err(err) => {
                    self.remember_error_locked(&err.to_string(), "io");
                    if attempt == 0 {
                        thread::sleep(Duration::from_millis(20));
                    }
                }
            }
        }
        false
    }

    fn truncate_to_max_lines_locked(&self) {
        let path = self.log_path();
        let Ok(content) = fs::read_to_string(&path) else {
            return;
        };
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() <= MAX_LINES {
            return;
        }
        let retained = lines[lines.len() - MAX_LINES..].join("\n") + "\n";
        if let Err(err) = fs::write(&path, retained) {
            self.remember_error_locked(&err.to_string(), "io");
        }
    }

    fn remember_error_locked(&self, message: &str, kind: &str) {
        if let Ok(mut diag) = self.diagnostics.lock() {
            diag.client_log_last_error = Some(message.into());
            diag.client_log_last_error_kind = Some(kind.into());
        }
    }

    fn mark_drop_locked(&self, reason: &str) {
        if let Ok(mut diag) = self.diagnostics.lock() {
            diag.client_log_events_dropped += 1;
            diag.client_log_last_error_kind = Some(reason.into());
        }
    }
}

fn utc_now_iso() -> String {
    voicesub_types::utc_now_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_appends_and_keeps_last_lines() {
        let dir = std::env::temp_dir().join(format!("vs-session-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let logger = SessionLogManager::new(&dir);
        for index in 0..3 {
            let result = logger.log(
                "dashboard",
                &format!("line-{index}"),
                Some("dashboard"),
                None,
            );
            assert!(result.logged);
        }
        let content = fs::read_to_string(logger.log_path()).unwrap();
        assert!(content.contains("line-2"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn empty_message_is_dropped() {
        let dir = std::env::temp_dir().join(format!("vs-session-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let logger = SessionLogManager::new(&dir);
        let result = logger.log("dashboard", "   ", None, None);
        assert!(!result.logged);
        assert_eq!(result.reason.as_deref(), Some("empty_message"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn compact_mode_filters_tts_client_log() {
        let _guard = crate::env_test::lock();
        crate::env_test::reset_compact_logging_env();
        let dir = std::env::temp_dir().join(format!("vs-session-tts-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let logger = SessionLogManager::new(&dir);
        let result = logger.log("tts", "engine.speak_end", Some("tts-window"), None);
        assert!(!result.logged);
        assert_eq!(result.reason.as_deref(), Some("compact_mode_filtered"));
        let _ = fs::remove_dir_all(dir);
    }
}
