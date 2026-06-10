//! Windows WASAPI enumeration and per-process endpoint routing.

use tracing::{debug, info, warn};
use wasapi::{get_default_device, DeviceCollection, Direction};

use crate::policy_config::set_persisted_process_endpoint;
use crate::trace;
use crate::{AudioError, AudioOutputDevice};

pub fn list_output_devices() -> Result<Vec<AudioOutputDevice>, AudioError> {
    debug!(target: "voicesub.tts.audio", "wasapi CoInitializeEx");
    unsafe {
        use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED};
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if hr.is_err() && hr != RPC_E_CHANGED_MODE {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
    }

    let mut devices = vec![AudioOutputDevice {
        id: String::new(),
        label: "Default".to_string(),
        is_default: true,
    }];

    if let Ok(default) = get_default_device(&Direction::Render) {
        let id = default.get_id().map_err(map_wasapi)?;
        let label = default
            .get_friendlyname()
            .unwrap_or_else(|_| "Default device".to_string());
        devices.push(AudioOutputDevice {
            id,
            label: format!("{label} (default)"),
            is_default: false,
        });
    }

    let collection = DeviceCollection::new(&Direction::Render).map_err(map_wasapi)?;
    let count = collection.get_nbr_devices().map_err(map_wasapi)?;
    for index in 0..count {
        let device = collection.get_device_at_index(index).map_err(map_wasapi)?;
        let id = device.get_id().map_err(map_wasapi)?;
        if devices.iter().any(|d| d.id == id) {
            continue;
        }
        let label = device
            .get_friendlyname()
            .unwrap_or_else(|_| format!("Output {}", index + 1));
        devices.push(AudioOutputDevice {
            id,
            label,
            is_default: false,
        });
    }

    Ok(devices)
}

pub fn set_process_output_device(pid: u32, device_id: &str) -> Result<(), AudioError> {
    if device_id.is_empty() {
        info!(target: "voicesub.tts.audio", pid, "using system default audio output");
        trace::trace(
            "platform",
            "route_default",
            serde_json::json!({ "pid": pid }),
        );
        return Ok(());
    }

    if !is_per_process_routing_enabled() {
        info!(
            target: "voicesub.tts.audio",
            pid,
            device_id,
            "per-process routing skipped (set VOICESUB_TTS_PER_PROCESS_ROUTING=1 to enable)"
        );
        trace::trace(
            "platform",
            "route_skipped_experimental",
            serde_json::json!({ "pid": pid, "device_id": device_id }),
        );
        return Ok(());
    }

    validate_device_id(device_id)?;
    set_persisted_process_endpoint(pid, device_id)?;
    info!(
        target: "voicesub.tts.audio",
        pid,
        device_id,
        "per-process audio output route applied"
    );
    trace::trace(
        "platform",
        "route_applied",
        serde_json::json!({
            "pid": pid,
            "device_id": device_id,
        }),
    );
    Ok(())
}

fn validate_device_id(device_id: &str) -> Result<(), AudioError> {
    let devices = list_output_devices()?;
    if devices.iter().any(|d| !d.id.is_empty() && d.id == device_id) {
        Ok(())
    } else {
        warn!(
            target: "voicesub.tts.audio",
            device_id,
            "unknown render endpoint id"
        );
        Err(AudioError::DeviceNotFound(device_id.to_string()))
    }
}

fn map_wasapi(err: wasapi::WasapiError) -> AudioError {
    AudioError::RoutingFailed(err.to_string())
}

/// Experimental WinAPI per-process routing (`IAudioPolicyConfigFactory`).
/// Disabled by default — use browser `selectAudioOutput()` in the TTS window instead.
pub fn is_per_process_routing_enabled() -> bool {
    matches!(
        std::env::var("VOICESUB_TTS_PER_PROCESS_ROUTING")
            .ok()
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("1") | Some("true") | Some("yes") | Some("on")
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_test_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn enumerate_returns_default_plus_hardware() {
        let devices = list_output_devices().expect("devices");
        assert!(devices.first().unwrap().is_default);
    }

    #[test]
    fn skips_unknown_device_when_routing_disabled() {
        let _guard = env_test_lock();
        std::env::remove_var("VOICESUB_TTS_PER_PROCESS_ROUTING");
        set_process_output_device(1234, "{not-a-real-endpoint}").expect("routing disabled");
    }

    #[test]
    fn validate_rejects_unknown_device_id_when_routing_enabled() {
        let _guard = env_test_lock();
        let _prev = std::env::var("VOICESUB_TTS_PER_PROCESS_ROUTING").ok();
        std::env::set_var("VOICESUB_TTS_PER_PROCESS_ROUTING", "1");
        let err = set_process_output_device(1234, "{not-a-real-endpoint}").unwrap_err();
        assert!(matches!(err, AudioError::DeviceNotFound(_)));
        if let Some(value) = _prev {
            std::env::set_var("VOICESUB_TTS_PER_PROCESS_ROUTING", value);
        } else {
            std::env::remove_var("VOICESUB_TTS_PER_PROCESS_ROUTING");
        }
    }
}
