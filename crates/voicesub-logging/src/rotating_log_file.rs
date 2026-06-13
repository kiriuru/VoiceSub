//! Append-only log file writer with size-based rotation (used by `core.log`).

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::log_rotation::{rotate_if_needed, DEFAULT_BACKUP_COUNT, DEFAULT_MAX_BYTES};

#[derive(Debug)]
pub struct RotatingLogFile {
    path: PathBuf,
    max_bytes: u64,
    backup_count: u32,
    lock: Mutex<()>,
}

impl RotatingLogFile {
    pub fn open(path: impl AsRef<Path>, max_bytes: u64, backup_count: u32) -> Arc<Self> {
        Arc::new(Self {
            path: path.as_ref().to_path_buf(),
            max_bytes,
            backup_count,
            lock: Mutex::new(()),
        })
    }

    pub fn write_all(&self, buf: &[u8]) -> io::Result<()> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        rotate_if_needed(&self.path, self.max_bytes, self.backup_count);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(buf)?;
        file.flush()
    }
}

pub struct RotatingLogFileWriter(Arc<RotatingLogFile>);

impl Write for RotatingLogFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn default_core_log_writer(path: impl AsRef<Path>) -> impl Fn() -> RotatingLogFileWriter + Send + Sync + 'static {
    let file = RotatingLogFile::open(path, DEFAULT_MAX_BYTES, DEFAULT_BACKUP_COUNT);
    move || RotatingLogFileWriter(file.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_lines() {
        let path = std::env::temp_dir().join(format!("vs-core-log-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        let file = RotatingLogFile::open(&path, DEFAULT_MAX_BYTES, DEFAULT_BACKUP_COUNT);
        file.write_all(b"line\n").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "line\n");
        let _ = fs::remove_file(path);
    }
}
