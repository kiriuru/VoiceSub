//! Windows process memory / handle telemetry for long-session diagnostics.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessResourceSnapshot {
    pub pid: u32,
    pub name: String,
    pub handle_count: u32,
    /// Private commit charge (bytes), closest to Task Manager "Commit".
    pub commit_bytes: u64,
    pub working_set_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTelemetry {
    pub self_process: ProcessResourceSnapshot,
    pub watched: Vec<ProcessResourceSnapshot>,
}

impl Default for ResourceTelemetry {
    fn default() -> Self {
        Self {
            self_process: ProcessResourceSnapshot {
                pid: 0,
                name: "unknown".to_string(),
                handle_count: 0,
                commit_bytes: 0,
                working_set_bytes: 0,
            },
            watched: Vec::new(),
        }
    }
}

const WATCHED_EXECUTABLES: &[&str] = &["voicesub-app.exe", "obs64.exe"];

#[cfg(windows)]
pub fn collect_resource_telemetry() -> ResourceTelemetry {
    let self_pid = std::process::id();
    let self_process = snapshot_process(self_pid).unwrap_or_else(|| ProcessResourceSnapshot {
        pid: self_pid,
        name: "tts-module".to_string(),
        handle_count: 0,
        commit_bytes: 0,
        working_set_bytes: 0,
    });

    let mut watched: Vec<ProcessResourceSnapshot> = Vec::new();
    for pid in enum_process_ids() {
        if pid == self_pid || pid == 0 {
            continue;
        }
        let Some(snapshot) = snapshot_process(pid) else {
            continue;
        };
        if !WATCHED_EXECUTABLES
            .iter()
            .any(|name| names_match(&snapshot.name, name))
        {
            continue;
        }
        if let Some(existing) = watched.iter_mut().find(|entry| names_match(&entry.name, &snapshot.name))
        {
            if snapshot.commit_bytes > existing.commit_bytes {
                *existing = snapshot;
            }
            continue;
        }
        watched.push(snapshot);
    }

    watched.sort_by(|a, b| a.name.cmp(&b.name));

    ResourceTelemetry {
        self_process,
        watched,
    }
}

#[cfg(not(windows))]
pub fn collect_resource_telemetry() -> ResourceTelemetry {
    ResourceTelemetry::default()
}

#[cfg(windows)]
fn names_match(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
}

#[cfg(windows)]
fn enum_process_ids() -> Vec<u32> {
    use windows::Win32::System::ProcessStatus::EnumProcesses;

    let mut buffer = vec![0u32; 2048];
    loop {
        let mut bytes_returned = 0u32;
        let result = unsafe {
            EnumProcesses(
                buffer.as_mut_ptr(),
                (buffer.len() * std::mem::size_of::<u32>()) as u32,
                &mut bytes_returned,
            )
        };
        if result.is_ok() {
            let count = bytes_returned as usize / std::mem::size_of::<u32>();
            buffer.truncate(count);
            return buffer;
        }
        if buffer.len() >= 65_536 {
            return Vec::new();
        }
        buffer.resize(buffer.len() * 2, 0);
    }
}

#[cfg(windows)]
fn snapshot_process(pid: u32) -> Option<ProcessResourceSnapshot> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let snapshot = read_process_snapshot(handle, pid);
        let _ = CloseHandle(handle);
        snapshot
    }
}

#[cfg(windows)]
unsafe fn read_process_snapshot(handle: windows::Win32::Foundation::HANDLE, pid: u32) -> Option<ProcessResourceSnapshot> {
    use std::mem::size_of;
    use windows::core::PWSTR;
    use windows::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows::Win32::System::Threading::{
        GetProcessHandleCount, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    };

    let mut name_buf = [0u16; 512];
    let mut name_len = name_buf.len() as u32;
    let name = if QueryFullProcessImageNameW(
        handle,
        PROCESS_NAME_WIN32,
        PWSTR(name_buf.as_mut_ptr()),
        &mut name_len,
    )
    .is_ok()
    {
        let path = String::from_utf16_lossy(&name_buf[..name_len as usize]);
        path.rsplit(['\\', '/'])
            .next()
            .unwrap_or("unknown")
            .to_string()
    } else {
        format!("pid-{pid}")
    };

    let mut handle_count = 0u32;
    let _ = GetProcessHandleCount(handle, &mut handle_count);

    let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
    counters.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    if GetProcessMemoryInfo(
        handle,
        &mut counters as *mut _ as *mut _,
        counters.cb,
    )
    .is_err()
    {
        return Some(ProcessResourceSnapshot {
            pid,
            name,
            handle_count,
            commit_bytes: 0,
            working_set_bytes: 0,
        });
    }

    Some(ProcessResourceSnapshot {
        pid,
        name,
        handle_count,
        commit_bytes: counters.PrivateUsage as u64,
        working_set_bytes: counters.WorkingSetSize as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_has_self_process_on_windows() {
        let telemetry = collect_resource_telemetry();
        #[cfg(windows)]
        {
            assert_eq!(telemetry.self_process.pid, std::process::id());
            assert!(!telemetry.self_process.name.is_empty());
        }
        #[cfg(not(windows))]
        {
            assert_eq!(telemetry.self_process.pid, 0);
        }
    }
}
