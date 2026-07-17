use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferPhase {
    Idle,
    Downloading,
    Extracting,
    Finalizing,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgress {
    pub active: bool,
    pub phase: TransferPhase,
    pub label: String,
    pub target: String,
    pub received_bytes: u64,
    pub total_bytes: Option<u64>,
    pub speed_bps: u64,
    pub percent: Option<f32>,
    pub error: Option<String>,
    pub cancelled: bool,
}

impl Default for TransferProgress {
    fn default() -> Self {
        Self {
            active: false,
            phase: TransferPhase::Idle,
            label: String::new(),
            target: String::new(),
            received_bytes: 0,
            total_bytes: None,
            speed_bps: 0,
            percent: None,
            error: None,
            cancelled: false,
        }
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("download cancelled")]
pub struct TransferCancelled;

#[derive(Clone, Default)]
struct TransferControl {
    cancel: Arc<AtomicBool>,
    cleanup_dirs: Arc<Mutex<Vec<PathBuf>>>,
    cleanup_files: Arc<Mutex<Vec<PathBuf>>>,
}

impl TransferControl {
    fn reset(&self) {
        self.cancel.store(false, Ordering::SeqCst);
        if let Ok(mut dirs) = self.cleanup_dirs.lock() {
            dirs.clear();
        }
        if let Ok(mut files) = self.cleanup_files.lock() {
            files.clear();
        }
    }

    fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    fn register_cleanup_dir(&self, path: PathBuf) {
        if let Ok(mut dirs) = self.cleanup_dirs.lock() {
            if !dirs.iter().any(|entry| entry == &path) {
                dirs.push(path);
            }
        }
    }

    fn register_cleanup_file(&self, path: PathBuf) {
        if let Ok(mut files) = self.cleanup_files.lock() {
            if !files.iter().any(|entry| entry == &path) {
                files.push(path);
            }
        }
    }

    fn execute_cleanup(&self) {
        let files = self
            .cleanup_files
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        for path in files {
            let _ = fs::remove_file(&path);
        }
        let dirs = self
            .cleanup_dirs
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        for path in dirs {
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path);
            } else if path.is_file() {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

#[derive(Clone)]
pub struct TransferTracker {
    inner: Arc<RwLock<TransferProgress>>,
    control: TransferControl,
}

impl TransferTracker {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(TransferProgress::default())),
            control: TransferControl::default(),
        }
    }

    pub fn snapshot(&self) -> TransferProgress {
        self.inner
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn reporter(&self) -> TransferReporter {
        TransferReporter {
            inner: Arc::clone(&self.inner),
            control: self.control.clone(),
            tick: Instant::now(),
            last_bytes: 0,
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = TransferProgress::default();
        }
        self.control.reset();
    }

    pub fn request_cancel(&self) -> bool {
        let active = self
            .inner
            .read()
            .map(|guard| guard.active)
            .unwrap_or(false);
        if !active {
            return false;
        }
        self.control.request_cancel();
        true
    }
}

impl Default for TransferTracker {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TransferReporter {
    inner: Arc<RwLock<TransferProgress>>,
    control: TransferControl,
    tick: Instant,
    last_bytes: u64,
}

impl TransferReporter {
    pub fn begin(&mut self, target: impl Into<String>, label: impl Into<String>) {
        self.control.reset();
        self.tick = Instant::now();
        self.last_bytes = 0;
        if let Ok(mut guard) = self.inner.write() {
            *guard = TransferProgress {
                active: true,
                phase: TransferPhase::Downloading,
                label: label.into(),
                target: target.into(),
                received_bytes: 0,
                total_bytes: None,
                speed_bps: 0,
                percent: None,
                error: None,
                cancelled: false,
            };
        }
    }

    pub fn set_phase(&self, phase: TransferPhase) {
        if let Ok(mut guard) = self.inner.write() {
            guard.phase = phase;
        }
    }

    pub fn set_total(&self, total: Option<u64>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.total_bytes = total.filter(|bytes| *bytes > 0);
            guard.percent = percent(guard.received_bytes, guard.total_bytes);
        }
    }

    pub fn set_label(&self, label: impl Into<String>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.label = label.into();
        }
    }

    pub fn register_cleanup_dir(&self, path: PathBuf) {
        self.control.register_cleanup_dir(path);
    }

    pub fn register_cleanup_file(&self, path: PathBuf) {
        self.control.register_cleanup_file(path);
    }

    pub fn is_cancelled(&self) -> bool {
        self.control.is_cancelled()
    }

    pub fn check_cancelled(&self) -> Result<(), TransferCancelled> {
        if self.control.is_cancelled() {
            Err(TransferCancelled)
        } else {
            Ok(())
        }
    }

    pub fn add_bytes(&mut self, delta: u64) {
        if delta == 0 {
            return;
        }
        let elapsed = self.tick.elapsed().as_secs_f64().max(0.001);
        if let Ok(mut guard) = self.inner.write() {
            guard.received_bytes = guard.received_bytes.saturating_add(delta);
            if guard
                .total_bytes
                .is_some_and(|total| guard.received_bytes > total)
            {
                guard.total_bytes = Some(guard.received_bytes);
            }
            let since_last = guard.received_bytes.saturating_sub(self.last_bytes);
            guard.speed_bps = (since_last as f64 / elapsed) as u64;
            guard.percent = percent(guard.received_bytes, guard.total_bytes);
        }
        self.tick = Instant::now();
        if let Ok(guard) = self.inner.read() {
            self.last_bytes = guard.received_bytes;
        }
    }

    pub fn finish_ok(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.active = false;
            guard.phase = TransferPhase::Idle;
            guard.speed_bps = 0;
            if guard.total_bytes.is_some() {
                guard.percent = Some(100.0);
            }
            guard.error = None;
            guard.cancelled = false;
        }
        self.control.reset();
    }

    pub fn finish_cancelled(&self) {
        self.control.execute_cleanup();
        if let Ok(mut guard) = self.inner.write() {
            guard.active = false;
            guard.phase = TransferPhase::Idle;
            guard.speed_bps = 0;
            guard.error = None;
            guard.cancelled = true;
        }
        self.control.reset();
    }

    pub fn finish_err(&self, message: impl Into<String>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.active = false;
            guard.phase = TransferPhase::Idle;
            guard.speed_bps = 0;
            guard.error = Some(message.into());
            guard.cancelled = false;
        }
        self.control.reset();
    }
}

fn percent(received: u64, total: Option<u64>) -> Option<f32> {
    let total = total?;
    if total == 0 {
        return None;
    }
    Some((received as f32 / total as f32 * 100.0).clamp(0.0, 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reporter_grows_total_when_received_exceeds_estimate() {
        let tracker = TransferTracker::new();
        let mut reporter = tracker.reporter();
        reporter.begin("cuda_redist", "CUDA runtime");
        reporter.set_total(Some(100));
        reporter.add_bytes(150);
        let snap = tracker.snapshot();
        assert_eq!(snap.total_bytes, Some(150));
        assert_eq!(snap.percent, Some(100.0));
    }

    #[test]
    fn set_total_ignores_zero() {
        let tracker = TransferTracker::new();
        let mut reporter = tracker.reporter();
        reporter.begin("ort_cpu", "ONNX Runtime (CPU)");
        reporter.set_total(Some(0));
        assert_eq!(tracker.snapshot().total_bytes, None);
        reporter.set_total(Some(200));
        reporter.add_bytes(50);
        assert_eq!(tracker.snapshot().percent, Some(25.0));
    }

    #[test]
    fn reporter_tracks_percent() {
        let tracker = TransferTracker::new();
        let mut reporter = tracker.reporter();
        reporter.begin("ort_cpu", "ONNX Runtime CPU");
        reporter.set_total(Some(100));
        reporter.add_bytes(25);
        let snap = tracker.snapshot();
        assert!(snap.active);
        assert_eq!(snap.received_bytes, 25);
        assert_eq!(snap.percent, Some(25.0));
        reporter.finish_ok();
        assert!(!tracker.snapshot().active);
    }

    #[test]
    fn cancel_runs_registered_cleanup() {
        let tracker = TransferTracker::new();
        let mut reporter = tracker.reporter();
        let dir = tempfile::tempdir().unwrap();
        let partial = dir.path().join("encoder.onnx.part");
        fs::write(&partial, b"partial").unwrap();
        reporter.begin("model:parakeet_tdt:int8", "Parakeet TDT int8");
        reporter.register_cleanup_file(partial.clone());
        reporter.register_cleanup_dir(dir.path().to_path_buf());
        tracker.request_cancel();
        reporter.finish_cancelled();
        assert!(!partial.is_file());
        assert!(!dir.path().is_dir());
        let snap = tracker.snapshot();
        assert!(snap.cancelled);
        assert!(!snap.active);
    }
}
