//! Prune orphaned `target/*/incremental/` directories left by rustc.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PruneOptions {
    pub if_needed_bytes: Option<u64>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneReport {
    pub scanned_dirs: usize,
    pub removed_dirs: usize,
    pub freed_bytes: u64,
    pub incremental_bytes_before: u64,
}

pub fn incremental_dir_size(root: &Path) -> io::Result<u64> {
    let mut total = 0u64;
    for profile in ["debug", "release"] {
        let incremental = root.join(profile).join("incremental");
        if !incremental.is_dir() {
            continue;
        }
        total = total.saturating_add(dir_tree_size(&incremental)?);
    }
    Ok(total)
}

/// Cursor agent sandboxes redirect `CARGO_TARGET_DIR` here; same incremental leak applies.
pub fn cursor_sandbox_roots() -> Vec<PathBuf> {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else {
        return Vec::new();
    };
    let cache = PathBuf::from(local_app_data)
        .join("Temp")
        .join("cursor-sandbox-cache");
    let Ok(entries) = fs::read_dir(&cache) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().join("cargo-target"))
        .filter(|path| path.is_dir())
        .collect()
}

#[allow(dead_code)]
pub fn prune_target(root: &Path, options: PruneOptions) -> io::Result<PruneReport> {
    prune_target_inner(root, options, true)
}

pub fn prune_dev_caches(workspace_target: Option<&Path>, options: PruneOptions) -> io::Result<PruneReport> {
    let mut roots: Vec<PathBuf> = cursor_sandbox_roots();
    if let Some(target) = workspace_target {
        if target.is_dir() {
            roots.push(target.to_path_buf());
        }
    }

    let combined_before: u64 = roots
        .iter()
        .map(|root| incremental_dir_size(root).unwrap_or(0))
        .sum();

    if let Some(threshold) = options.if_needed_bytes {
        let sandbox_bulk = sandbox_dirs_over(threshold);
        if combined_before < threshold && sandbox_bulk.is_empty() {
            return Ok(PruneReport {
                scanned_dirs: 0,
                removed_dirs: 0,
                freed_bytes: 0,
                incremental_bytes_before: combined_before,
            });
        }
    }

    let mut report = PruneReport {
        scanned_dirs: 0,
        removed_dirs: 0,
        freed_bytes: 0,
        incremental_bytes_before: combined_before,
    };

    for root in roots {
        let part = prune_target_inner(&root, options, false)?;
        report.scanned_dirs += part.scanned_dirs;
        report.removed_dirs += part.removed_dirs;
        report.freed_bytes = report.freed_bytes.saturating_add(part.freed_bytes);
    }

    if let Some(threshold) = options.if_needed_bytes {
        for path in sandbox_dirs_over(threshold) {
            let size = dir_tree_size(&path)?;
            if options.dry_run {
                eprintln!(
                    "[dry-run] would remove cursor sandbox {} ({size} bytes)",
                    path.display()
                );
            } else {
                fs::remove_dir_all(&path)?;
            }
            report.removed_dirs += 1;
            report.freed_bytes = report.freed_bytes.saturating_add(size);
        }
    }

    Ok(report)
}

fn sandbox_dirs_over(threshold: u64) -> Vec<PathBuf> {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else {
        return Vec::new();
    };
    let cache = PathBuf::from(local_app_data)
        .join("Temp")
        .join("cursor-sandbox-cache");
    let Ok(entries) = fs::read_dir(&cache) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let size = dir_tree_size(&path).ok()?;
            (size >= threshold).then_some(path)
        })
        .collect()
}

fn prune_target_inner(
    root: &Path,
    options: PruneOptions,
    honor_if_needed: bool,
) -> io::Result<PruneReport> {
    let before = incremental_dir_size(root)?;
    if honor_if_needed {
        if let Some(threshold) = options.if_needed_bytes {
            if before < threshold {
                return Ok(PruneReport {
                    scanned_dirs: 0,
                    removed_dirs: 0,
                    freed_bytes: 0,
                    incremental_bytes_before: before,
                });
            }
        }
    }

    let mut scanned_dirs = 0usize;
    let mut removed_dirs = 0usize;
    let mut freed_bytes = 0u64;

    for profile in ["debug", "release"] {
        let incremental = root.join(profile).join("incremental");
        if !incremental.is_dir() {
            continue;
        }
        let plan = plan_incremental_prune(&incremental)?;
        scanned_dirs += plan.scanned_dirs;
        for path in plan.remove {
            let size = dir_tree_size(&path)?;
            if options.dry_run {
                eprintln!("[dry-run] would remove {} ({size} bytes)", path.display());
            } else {
                fs::remove_dir_all(&path)?;
            }
            removed_dirs += 1;
            freed_bytes = freed_bytes.saturating_add(size);
        }
    }

    Ok(PruneReport {
        scanned_dirs,
        removed_dirs,
        freed_bytes,
        incremental_bytes_before: before,
    })
}

#[derive(Debug)]
struct PrunePlan {
    scanned_dirs: usize,
    remove: Vec<PathBuf>,
}

fn plan_incremental_prune(incremental: &Path) -> io::Result<PrunePlan> {
    let mut groups: HashMap<String, Vec<(PathBuf, SystemTime)>> = HashMap::new();

    for entry in fs::read_dir(incremental)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() {
            continue;
        }
        let path = entry.path();
        let Some(stem) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(crate_stem) = incremental_crate_stem(stem) else {
            continue;
        };
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        groups
            .entry(crate_stem)
            .or_default()
            .push((path, modified));
    }

    let mut scanned_dirs = 0usize;
    let mut remove = Vec::new();
    for mut entries in groups.into_values() {
        if entries.len() <= 1 {
            scanned_dirs += entries.len();
            continue;
        }
        entries.sort_by_key(|(_, modified)| *modified);
        scanned_dirs += entries.len();
        let stale_count = entries.len().saturating_sub(1);
        for (path, _) in entries.into_iter().take(stale_count) {
            remove.push(path);
        }
    }

    Ok(PrunePlan {
        scanned_dirs,
        remove,
    })
}

fn incremental_crate_stem(dir_name: &str) -> Option<String> {
    let (stem, suffix) = dir_name.rsplit_once('-')?;
    // rustc incremental hashes are long alphanumeric tokens, not short words.
    if suffix.len() < 8 || !suffix.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    if stem.is_empty() {
        return None;
    }
    Some(stem.to_string())
}

fn dir_tree_size(path: &Path) -> io::Result<u64> {
    let mut total = 0u64;
    if path.is_file() {
        return Ok(path.metadata()?.len());
    }
    if !path.is_dir() {
        return Ok(0);
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            total = total.saturating_add(dir_tree_size(&entry.path())?);
        } else if file_type.is_file() {
            total = total.saturating_add(entry.metadata()?.len());
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn incremental_crate_stem_parses_hash_suffix() {
        assert_eq!(
            incremental_crate_stem("voicesub_app_lib-2yn0nus3gg001").as_deref(),
            Some("voicesub_app_lib")
        );
        assert!(incremental_crate_stem("xtask-0abc12").is_none());
        assert!(incremental_crate_stem("no-hash-here").is_none());
    }

    #[test]
    fn prune_keeps_newest_incremental_dir_per_crate() {
        let root = std::env::temp_dir().join(format!(
            "voicesub-prune-test-{}",
            std::process::id()
        ));
        let incremental = root.join("debug").join("incremental");
        let old = incremental.join("voicesub_app_lib-oldhash111");
        let new = incremental.join("voicesub_app_lib-newhash222");
        fs::create_dir_all(&old).expect("old dir");
        fs::create_dir_all(&new).expect("new dir");
        fs::write(old.join("artifact.bin"), vec![0u8; 32]).expect("old file");
        thread::sleep(Duration::from_millis(20));
        fs::write(new.join("artifact.bin"), vec![0u8; 64]).expect("new file");

        let report = prune_target(
            &root,
            PruneOptions {
                if_needed_bytes: None,
                dry_run: false,
            },
        )
        .expect("prune");

        assert_eq!(report.removed_dirs, 1);
        assert!(!old.exists());
        assert!(new.exists());

        fs::remove_dir_all(&root).ok();
    }
}
