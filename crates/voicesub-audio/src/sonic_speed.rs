//! Speech tempo change via vendored libsonic (pitch-preserving, low latency for TTS).

use std::os::raw::c_int;

use crate::error::AudioError;

type SonicStream = *mut std::ffi::c_void;

extern "C" {
    fn sonicCreateStream(sample_rate: c_int, num_channels: c_int) -> SonicStream;
    fn sonicDestroyStream(stream: SonicStream);
    fn sonicSetSpeed(stream: SonicStream, speed: f32);
    fn sonicSetPitch(stream: SonicStream, pitch: f32);
    fn sonicSetQuality(stream: SonicStream, quality: c_int);
    fn sonicWriteFloatToStream(stream: SonicStream, samples: *const f32, num_samples: c_int) -> c_int;
    fn sonicFlushStream(stream: SonicStream) -> c_int;
    fn sonicSamplesAvailable(stream: SonicStream) -> c_int;
    fn sonicReadFloatFromStream(stream: SonicStream, samples: *mut f32, max_samples: c_int) -> c_int;
}

/// Change playback tempo without altering pitch (`speed` = `speech_rate`).
pub fn change_speech_speed(
    pcm: &[f32],
    sample_rate: u32,
    channels: u16,
    speed: f32,
) -> Result<Vec<f32>, AudioError> {
    if pcm.is_empty() {
        return Err(AudioError::PlaybackFailed("decoded audio is empty".into()));
    }
    if channels == 0 || sample_rate == 0 {
        return Err(AudioError::PlaybackFailed("invalid audio format".into()));
    }

    unsafe {
        let stream = sonicCreateStream(sample_rate as c_int, channels as c_int);
        if stream.is_null() {
            return Err(AudioError::PlaybackFailed("sonic stream init failed".into()));
        }

        let result = change_speech_speed_on_stream(stream, pcm, speed);
        sonicDestroyStream(stream);
        result
    }
}

unsafe fn change_speech_speed_on_stream(
    stream: SonicStream,
    pcm: &[f32],
    speed: f32,
) -> Result<Vec<f32>, AudioError> {
    sonicSetSpeed(stream, speed);
    sonicSetPitch(stream, 1.0);
    sonicSetQuality(stream, 0);

    if sonicWriteFloatToStream(stream, pcm.as_ptr(), pcm.len() as c_int) == 0 {
        return Err(AudioError::PlaybackFailed("sonic write failed".into()));
    }
    if sonicFlushStream(stream) == 0 {
        return Err(AudioError::PlaybackFailed("sonic flush failed".into()));
    }

    let available = sonicSamplesAvailable(stream);
    if available <= 0 {
        return Err(AudioError::PlaybackFailed("sonic produced no output".into()));
    }

    let mut out = vec![0.0f32; available as usize];
    let read = sonicReadFloatFromStream(stream, out.as_mut_ptr(), available);
    if read <= 0 {
        return Err(AudioError::PlaybackFailed("sonic read failed".into()));
    }
    out.truncate(read as usize);
    Ok(out)
}
