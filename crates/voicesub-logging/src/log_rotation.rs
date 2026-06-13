//! Size-based log rotation shared by backbone and JSONL trace files.

use std::fs;
use std::path::Path;

pub const DEFAULT_MAX_BYTES: u64 = 5 * 1024 * 1024;
pub const DEFAULT_BACKUP_COUNT: u32 = 2;

pub fn rotate_if_needed(path: &Path, max_bytes: u64, backup_count: u32) {
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    if meta.len() < max_bytes {
        return;
    }
    for index in (1..=backup_count).rev() {
        let rotated = path.with_extension(format!("log.{index}"));
        if !rotated.exists() {
            continue;
        }
        if index >= backup_count {
            let _ = fs::remove_file(&rotated);
        } else {
            let next = path.with_extension(format!("log.{}", index + 1));
            let _ = fs::rename(&rotated, next);
        }
    }
    let first = path.with_extension("log.1");
    let _ = fs::rename(path, first);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("vs-log-rotate-{name}-{}", std::process::id()))
    }

    #[test]
    fn rotates_when_over_limit() {
        let path = temp_path("core");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("log.1"));
        {
            let mut file = fs::File::create(&path).unwrap();
            write!(file, "{}", "x".repeat(64)).unwrap();
        }
        rotate_if_needed(&path, 32, DEFAULT_BACKUP_COUNT);
        assert!(!path.is_file());
        assert!(path.with_extension("log.1").is_file());
        let _ = fs::remove_file(path.with_extension("log.1"));
    }
}
