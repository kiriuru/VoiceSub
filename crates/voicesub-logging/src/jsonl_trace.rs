use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::log_rotation::{rotate_if_needed, DEFAULT_BACKUP_COUNT, DEFAULT_MAX_BYTES};

pub struct JsonlTraceLog {
    path: PathBuf,
    session_id: String,
    sequence: Mutex<u64>,
    write_lock: Mutex<()>,
}

impl JsonlTraceLog {
    pub fn open(logs_dir: &Path, file_name: &str) -> Self {
        let _ = fs::create_dir_all(logs_dir);
        let path = logs_dir.join(file_name);
        let session_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs().to_string())
            .unwrap_or_else(|_| "0".into());
        let _ = fs::write(&path, "");
        Self {
            path,
            session_id,
            sequence: Mutex::new(0),
            write_lock: Mutex::new(()),
        }
    }

    pub fn append(&self, lane: &str, component: &str, event: &str, fields: Value) {
        let mut sequence = self.sequence.lock().unwrap_or_else(|e| e.into_inner());
        *sequence += 1;
        let record = json!({
            "timestamp_utc": utc_now_iso(),
            "session_id": self.session_id,
            "sequence": *sequence,
            "lane": lane,
            "component": component,
            "event": event,
            "fields": fields,
        });
        let line = match serde_json::to_string(&record) {
            Ok(text) => text,
            Err(_) => return,
        };
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        rotate_if_needed(&self.path, DEFAULT_MAX_BYTES, DEFAULT_BACKUP_COUNT);
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = writeln!(file, "{line}");
        }
    }
}

fn utc_now_iso() -> String {
    use std::time::Duration;
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    format!("{secs}")
}
