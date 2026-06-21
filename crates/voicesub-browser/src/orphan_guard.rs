//! Persist the launched browser worker PID so a leftover high-priority Chrome from a
//! previous **crashed** session can be reaped on next startup (review §8).
//!
//! The worker is launched detached in its own process group; if VoiceSub exits without a
//! graceful stop, `taskkill /T /F` is never issued and the worker keeps consuming CPU.
//! On the next launch we read the persisted PID and terminate it — but only after verifying
//! the live process is still a Chromium-family image, to avoid killing an unrelated process
//! that happened to reuse the PID.

use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

const PID_FILE_NAME: &str = "browser-worker.pid";

/// Location of the persisted worker PID file under the runtime data root.
pub fn pid_file_path(runtime_root: &Path) -> PathBuf {
    runtime_root.join(PID_FILE_NAME)
}

/// Record the live worker PID. Best-effort: failures are logged, never fatal.
pub fn record_worker_pid(runtime_root: &Path, pid: u32) {
    if pid == 0 {
        return;
    }
    let path = pid_file_path(runtime_root);
    match std::fs::write(&path, pid.to_string()) {
        Ok(()) => debug!(pid, path = %path.display(), "recorded browser worker pid"),
        Err(err) => warn!(pid, error = %err, "failed to record browser worker pid"),
    }
}

/// Remove the persisted PID file after a graceful stop.
pub fn clear_worker_pid(runtime_root: &Path) {
    let path = pid_file_path(runtime_root);
    match std::fs::remove_file(&path) {
        Ok(()) => debug!(path = %path.display(), "cleared browser worker pid file"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => warn!(error = %err, "failed to clear browser worker pid file"),
    }
}

/// Read a persisted PID (if any). Tolerates whitespace; ignores empty/garbage content.
pub fn read_persisted_pid(runtime_root: &Path) -> Option<u32> {
    let path = pid_file_path(runtime_root);
    let raw = std::fs::read_to_string(&path).ok()?;
    parse_pid(&raw)
}

fn parse_pid(raw: &str) -> Option<u32> {
    raw.trim().parse::<u32>().ok().filter(|pid| *pid != 0)
}

/// Image names that identify the Browser Speech worker process. PID-reuse guard: we only
/// reap a leftover PID if the live process is one of these.
pub fn is_worker_image(image: &str) -> bool {
    let lower = image.trim().to_ascii_lowercase();
    matches!(lower.as_str(), "chrome.exe" | "msedge.exe")
}

/// Parse the image name (first column) from a `tasklist /FO CSV /NH` line, e.g.
/// `"chrome.exe","1234","Console","1","250,000 K"` -> `chrome.exe`.
pub fn parse_tasklist_image_name(csv_line: &str) -> Option<String> {
    let trimmed = csv_line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let first = trimmed.split(',').next()?;
    let name = first.trim().trim_matches('"').trim();
    if name.is_empty() || name.starts_with("INFO:") {
        None
    } else {
        Some(name.to_string())
    }
}

/// Reap a leftover worker from a previous crashed session, if the PID file points at a live
/// Chromium-family process. Always clears the PID file afterwards.
pub fn reap_orphan_worker(runtime_root: &Path) {
    let Some(pid) = read_persisted_pid(runtime_root) else {
        return;
    };
    if let Some(image) = live_process_image(pid) {
        if is_worker_image(&image) {
            if crate::BrowserWorkerLauncher::terminate_worker(pid) {
                info!(
                    pid,
                    image, "reaped orphaned browser worker from previous session"
                );
            } else {
                warn!(pid, image, "failed to reap orphaned browser worker");
            }
        } else {
            debug!(
                pid,
                image, "persisted worker pid reused by unrelated process; not reaping"
            );
        }
    }
    clear_worker_pid(runtime_root);
}

#[cfg(windows)]
fn live_process_image(pid: u32) -> Option<String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().find_map(parse_tasklist_image_name)
}

#[cfg(not(windows))]
fn live_process_image(_pid: u32) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pid_tolerates_whitespace_and_rejects_zero() {
        assert_eq!(parse_pid(" 4321\n"), Some(4321));
        assert_eq!(parse_pid("0"), None);
        assert_eq!(parse_pid(""), None);
        assert_eq!(parse_pid("not-a-pid"), None);
    }

    #[test]
    fn worker_image_matches_chromium_family_case_insensitive() {
        assert!(is_worker_image("chrome.exe"));
        assert!(is_worker_image("CHROME.EXE"));
        assert!(is_worker_image("msedge.exe"));
        assert!(!is_worker_image("notepad.exe"));
        assert!(!is_worker_image(""));
    }

    #[test]
    fn tasklist_csv_image_name_is_extracted() {
        let line = "\"chrome.exe\",\"1234\",\"Console\",\"1\",\"250,000 K\"";
        assert_eq!(
            parse_tasklist_image_name(line).as_deref(),
            Some("chrome.exe")
        );
        assert_eq!(parse_tasklist_image_name("").as_deref(), None);
        assert_eq!(
            parse_tasklist_image_name("INFO: No tasks are running which match the criteria.")
                .as_deref(),
            None
        );
    }

    #[test]
    fn record_read_clear_roundtrip() {
        let dir = std::env::temp_dir().join(format!("voicesub-orphan-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        record_worker_pid(&dir, 12_345);
        assert_eq!(read_persisted_pid(&dir), Some(12_345));
        clear_worker_pid(&dir);
        assert_eq!(read_persisted_pid(&dir), None);
        // pid 0 is never recorded.
        record_worker_pid(&dir, 0);
        assert_eq!(read_persisted_pid(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
