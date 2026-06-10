//! Windows EcoQoS / power throttling opt-out for Chrome worker (SST port).

use tracing::warn;

#[cfg(windows)]
pub fn opt_out_chrome_power_throttling(pid: u32) {
    if pid == 0 {
        return;
    }

    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, ProcessPowerThrottling, SetProcessInformation,
        PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        PROCESS_POWER_THROTTLING_STATE, PROCESS_SET_INFORMATION,
    };

    unsafe {
        let handle = match OpenProcess(PROCESS_SET_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(err) => {
                warn!(pid, error = %err, "EcoQoS opt-out: OpenProcess failed");
                return;
            }
        };

        let mut state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
            StateMask: 0,
        };

        let ok = SetProcessInformation(
            handle,
            ProcessPowerThrottling,
            &mut state as *mut _ as *mut _,
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        );

        let _ = CloseHandle(handle);

        if ok.is_err() {
            warn!(pid, "EcoQoS opt-out: SetProcessInformation failed");
        }
    }
}

#[cfg(not(windows))]
pub fn opt_out_chrome_power_throttling(_pid: u32) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecoqos_no_panic_on_zero_pid() {
        opt_out_chrome_power_throttling(0);
    }
}
