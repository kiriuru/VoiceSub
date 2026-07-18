//! Best-effort CPU affinity for the Chrome ASR worker (Windows).

use tracing::{debug, warn};

const ENV_AFFINITY: &str = "VOICESUB_BROWSER_AFFINITY";
const ENV_AFFINITY_MASK: &str = "VOICESUB_BROWSER_AFFINITY_MASK";
const ENV_EXCLUDE_LOW: &str = "VOICESUB_BROWSER_AFFINITY_EXCLUDE_LOW";

fn env_truthy(name: &str) -> Option<bool> {
    match std::env::var(name).ok().as_deref() {
        Some("1") | Some("true") | Some("yes") | Some("on") => Some(true),
        Some("0") | Some("false") | Some("no") | Some("off") => Some(false),
        _ => None,
    }
}

fn parse_usize_env(name: &str) -> Option<usize> {
    std::env::var(name).ok().and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(hex) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            usize::from_str_radix(hex, 16).ok()
        } else {
            trimmed.parse().ok()
        }
    })
}

/// Resolve affinity mask for the browser worker process.
///
/// Opt-in: requires `VOICESUB_BROWSER_AFFINITY=1` (or truthy), or an explicit
/// `VOICESUB_BROWSER_AFFINITY_MASK`. Disabled by default.
pub fn resolve_browser_worker_affinity_mask() -> Option<usize> {
    if env_truthy(ENV_AFFINITY) == Some(false) {
        return None;
    }

    if let Some(mask) = parse_usize_env(ENV_AFFINITY_MASK) {
        return (mask > 0).then_some(mask);
    }

    // Default off unless explicitly enabled.
    if env_truthy(ENV_AFFINITY) != Some(true) {
        return None;
    }

    let exclude_low = parse_usize_env(ENV_EXCLUDE_LOW).unwrap_or(2);
    if exclude_low == 0 {
        return None;
    }

    let cores = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(0);
    if cores <= exclude_low + 1 {
        return None;
    }

    let all_bits = if cores >= usize::BITS as usize {
        usize::MAX
    } else {
        (1usize << cores) - 1
    };
    let reserved = (1usize << exclude_low.min(usize::BITS as usize - 1)) - 1;
    let mask = all_bits & !reserved;
    (mask > 0).then_some(mask)
}

/// Apply CPU affinity to a launched browser worker (no-op when disabled / unsupported).
pub fn apply_browser_worker_affinity(pid: u32) {
    if pid == 0 {
        return;
    }
    let Some(mask) = resolve_browser_worker_affinity_mask() else {
        return;
    };

    #[cfg(windows)]
    {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_SET_INFORMATION, SetProcessAffinityMask,
        };

        unsafe {
            let handle = match OpenProcess(PROCESS_SET_INFORMATION, false, pid) {
                Ok(handle) => handle,
                Err(err) => {
                    warn!(pid, error = %err, "browser worker affinity: OpenProcess failed");
                    return;
                }
            };
            let ok = SetProcessAffinityMask(handle, mask);
            let _ = CloseHandle(handle);
            if ok.is_err() {
                warn!(
                    pid,
                    mask, "browser worker affinity: SetProcessAffinityMask failed"
                );
            } else {
                debug!(pid, mask, "browser worker affinity applied");
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = mask;
        let _ = pid;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affinity_mask_resolution_cases() {
        unsafe {
            std::env::remove_var(ENV_AFFINITY);
            std::env::remove_var(ENV_AFFINITY_MASK);
            std::env::remove_var(ENV_EXCLUDE_LOW);
        }
        // Default: affinity disabled.
        assert!(resolve_browser_worker_affinity_mask().is_none());
        unsafe {
            std::env::set_var(ENV_AFFINITY, "0");
        }
        assert!(resolve_browser_worker_affinity_mask().is_none());
        unsafe {
            std::env::remove_var(ENV_AFFINITY);
            std::env::set_var(ENV_AFFINITY_MASK, "0xFC");
        }
        assert_eq!(resolve_browser_worker_affinity_mask(), Some(0xFC));
        unsafe {
            std::env::remove_var(ENV_AFFINITY_MASK);
            std::env::set_var(ENV_AFFINITY, "1");
            std::env::set_var(ENV_EXCLUDE_LOW, "2");
        }
        let cores = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(0);
        if cores <= 3 {
            assert!(resolve_browser_worker_affinity_mask().is_none());
        } else {
            let mask = resolve_browser_worker_affinity_mask().expect("mask");
            assert_eq!(mask & 0b11, 0);
        }
        unsafe {
            std::env::remove_var(ENV_AFFINITY);
            std::env::remove_var(ENV_EXCLUDE_LOW);
        }
    }
}
