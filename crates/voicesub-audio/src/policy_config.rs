//! Per-process audio endpoint routing via undocumented `IAudioPolicyConfigFactory`
//! (EarTrumpet / SoundSwitch pattern).

use std::ffi::c_void;
use std::ptr;

use tracing::{debug, info, warn};
use windows::core::{Interface, GUID, HSTRING, HRESULT, IUnknown, PCWSTR};
use windows::Win32::Media::Audio::{EDataFlow, ERole};

use crate::trace;
use crate::AudioError;

const IID_POLICY_CONFIG_21H2: GUID = GUID::from_values(
    0xab3d4648,
    0xe242,
    0x459f,
    [0xb0, 0x2f, 0x54, 0x1c, 0x70, 0x30, 0x63, 0x24],
);

const IID_POLICY_CONFIG_DOWNLEVEL: GUID = GUID::from_values(
    0x2a59116d,
    0x6c4f,
    0x45e0,
    [0xa7, 0x4f, 0x70, 0x7e, 0x3f, 0xef, 0x92, 0x58],
);

/// EarTrumpet pads IInspectable stubs before `SetPersistedDefaultAudioEndpoint`.
const SET_PERSISTED_VTABLE_INDEX: usize = 25;

type SetPersistedFn = unsafe extern "system" fn(
    factory: *mut c_void,
    process_id: u32,
    flow: EDataFlow,
    role: ERole,
    device_id: PCWSTR,
) -> HRESULT;

struct PolicyFactoryBinding {
    factory: IUnknown,
}

impl PolicyFactoryBinding {
    fn activate() -> Result<Self, AudioError> {
        let class_name = HSTRING::from("Windows.Media.Internal.AudioPolicyConfig");
        for iid in [IID_POLICY_CONFIG_21H2, IID_POLICY_CONFIG_DOWNLEVEL] {
            match ro_get_activation_factory(&class_name, &iid) {
                Ok(factory) => {
                    debug!(
                        target: "voicesub.tts.audio",
                        "IAudioPolicyConfigFactory activated"
                    );
                    return Ok(Self { factory });
                }
                Err(err) => {
                    debug!(
                        target: "voicesub.tts.audio",
                        error = %err,
                        "IAudioPolicyConfigFactory activation attempt failed"
                    );
                }
            }
        }

        warn!(
            target: "voicesub.tts.audio",
            "IAudioPolicyConfigFactory unavailable for known IIDs"
        );
        trace::trace(
            "policy_config",
            "factory_activate_failed",
            serde_json::json!({}),
        );
        Err(AudioError::RoutingFailed(
            "IAudioPolicyConfigFactory unavailable on this Windows build".to_string(),
        ))
    }

    unsafe fn set_persisted(
        &self,
        process_id: u32,
        device_id: &str,
    ) -> Result<(), AudioError> {
        let factory = self.factory.as_raw();
        let vtable = *(factory as *const *const *const c_void);
        let set_persisted =
            std::mem::transmute::<*const c_void, SetPersistedFn>(*vtable.add(SET_PERSISTED_VTABLE_INDEX));
        let wide = widestring(device_id);
        let hr = set_persisted(
            factory,
            process_id,
            windows::Win32::Media::Audio::eRender,
            windows::Win32::Media::Audio::eConsole,
            PCWSTR(wide.as_ptr()),
        );
        if hr.is_err() {
            let message = format!(
                "SetPersistedDefaultAudioEndpoint(pid={process_id}) failed: {hr:?}"
            );
            warn!(target: "voicesub.tts.audio", process_id, error = %message, "policy config routing failed");
            trace::trace(
                "policy_config",
                "set_persisted_failed",
                serde_json::json!({
                    "process_id": process_id,
                    "device_id": device_id,
                    "error": message,
                }),
            );
            return Err(AudioError::RoutingFailed(message));
        }
        Ok(())
    }
}

fn ro_get_activation_factory(class_id: &HSTRING, iid: &GUID) -> Result<IUnknown, AudioError> {
    windows_link::link!(
        "api-ms-win-core-winrt-l1-1-0.dll"
        "system"
        fn RoGetActivationFactory(
            activatableclassid: *mut c_void,
            iid: *const GUID,
            factory: *mut *mut c_void,
        ) -> HRESULT
    );

    let mut factory: *mut c_void = ptr::null_mut();
    unsafe {
        let hr = RoGetActivationFactory(
            std::mem::transmute_copy(class_id),
            iid,
            &mut factory,
        );
        if hr.is_err() || factory.is_null() {
            return Err(AudioError::RoutingFailed(format!(
                "RoGetActivationFactory failed: {hr:?}"
            )));
        }
        Ok(IUnknown::from_raw(factory))
    }
}

/// Apply persisted render endpoint for a specific process id.
pub fn set_persisted_process_endpoint(
    process_id: u32,
    device_id: &str,
) -> Result<(), AudioError> {
    if process_id == 0 {
        return Err(AudioError::InvalidProcessId);
    }

    debug!(
        target: "voicesub.tts.audio",
        process_id,
        device_id,
        "SetPersistedDefaultAudioEndpoint"
    );

    let binding = PolicyFactoryBinding::activate()?;
    unsafe { binding.set_persisted(process_id, device_id)? };
    info!(
        target: "voicesub.tts.audio",
        process_id,
        device_id,
        "policy config routing succeeded"
    );
    trace::trace(
        "policy_config",
        "set_persisted_ok",
        serde_json::json!({
            "process_id": process_id,
            "device_id": device_id,
        }),
    );
    Ok(())
}

fn widestring(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
