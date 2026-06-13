//! Pitch-preserving speech tempo via vendored libsonic (PICOLA / TD-PSOLA).
//!
//! Used by sonic playback mode instead of browser `HTMLAudioElement.preservesPitch` or
//! rodio `Sink::set_speed` (which shifts pitch).

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

/// Incremental libsonic processor for chunked MP3 decode + playback.
pub struct SonicProcessor {
    stream: SonicStream,
}

impl SonicProcessor {
    pub fn new(sample_rate: u32, channels: u16, speed: f32) -> Result<Self, AudioError> {
        if channels == 0 || sample_rate == 0 {
            return Err(AudioError::PlaybackFailed("invalid audio format".into()));
        }
        unsafe {
            let stream = sonicCreateStream(sample_rate as c_int, channels as c_int);
            if stream.is_null() {
                return Err(AudioError::PlaybackFailed("sonic stream init failed".into()));
            }
            sonicSetSpeed(stream, speed);
            sonicSetPitch(stream, 1.0);
            sonicSetQuality(stream, 0);
            Ok(Self { stream })
        }
    }

    pub fn write(&mut self, pcm: &[f32]) -> Result<(), AudioError> {
        if pcm.is_empty() {
            return Ok(());
        }
        unsafe {
            if sonicWriteFloatToStream(self.stream, pcm.as_ptr(), pcm.len() as c_int) == 0 {
                return Err(AudioError::PlaybackFailed("sonic write failed".into()));
            }
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), AudioError> {
        unsafe {
            if sonicFlushStream(self.stream) == 0 {
                return Err(AudioError::PlaybackFailed("sonic flush failed".into()));
            }
        }
        Ok(())
    }

    /// Read up to `max_samples` interleaved floats currently buffered by libsonic.
    pub fn drain_up_to(&mut self, max_samples: i32) -> Result<Vec<f32>, AudioError> {
        unsafe {
            let available = sonicSamplesAvailable(self.stream);
            if available <= 0 {
                return Ok(Vec::new());
            }
            let to_read = available.min(max_samples.max(1));
            let mut out = vec![0.0f32; to_read as usize];
            let read = sonicReadFloatFromStream(self.stream, out.as_mut_ptr(), to_read);
            if read <= 0 {
                return Ok(Vec::new());
            }
            out.truncate(read as usize);
            Ok(out)
        }
    }

    pub fn drain_all(&mut self) -> Result<Vec<f32>, AudioError> {
        let mut out = Vec::new();
        loop {
            let chunk = self.drain_up_to(16_384)?;
            if chunk.is_empty() {
                break;
            }
            out.extend(chunk);
        }
        if out.is_empty() {
            return Err(AudioError::PlaybackFailed("sonic produced no output".into()));
        }
        Ok(out)
    }
}

impl Drop for SonicProcessor {
    fn drop(&mut self) {
        unsafe {
            if !self.stream.is_null() {
                sonicDestroyStream(self.stream);
                self.stream = std::ptr::null_mut();
            }
        }
    }
}

/// Batch tempo change (tests + parity reference). Playback uses [`SonicProcessor`] incrementally.
#[allow(dead_code)]
pub fn change_speech_speed(
    pcm: &[f32],
    sample_rate: u32,
    channels: u16,
    speed: f32,
) -> Result<Vec<f32>, AudioError> {
    if pcm.is_empty() {
        return Err(AudioError::PlaybackFailed("decoded audio is empty".into()));
    }
    let mut sonic = SonicProcessor::new(sample_rate, channels, speed)?;
    sonic.write(pcm)?;
    sonic.flush()?;
    sonic.drain_all()
}

#[cfg(test)]
mod tests {
    use super::{change_speech_speed, SonicProcessor};

    #[test]
    fn sonic_shortens_for_faster_rate() {
        let input: Vec<f32> = (0..4410)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let output = change_speech_speed(&input, 44100, 1, 1.5).expect("sonic");
        assert!(output.len() < input.len());
    }

    #[test]
    fn sonic_lengthens_for_slower_rate() {
        let input: Vec<f32> = (0..4410)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let output = change_speech_speed(&input, 44100, 1, 0.75).expect("sonic");
        assert!(output.len() > input.len());
    }

    #[test]
    fn incremental_drain_matches_batch_after_flush() {
        let input: Vec<f32> = (0..4410)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let batch = change_speech_speed(&input, 44100, 1, 1.25).expect("batch");

        let mut sonic = SonicProcessor::new(44100, 1, 1.25).expect("sonic");
        sonic.write(&input).expect("write");
        sonic.flush().expect("flush");
        let mut streamed = Vec::new();
        loop {
            let piece = sonic.drain_up_to(16_384).expect("drain");
            if piece.is_empty() {
                break;
            }
            streamed.extend(piece);
        }

        assert_eq!(batch.len(), streamed.len());
        for (a, b) in batch.iter().zip(streamed.iter()) {
            assert!((a - b).abs() < 1e-5, "sample mismatch: {a} vs {b}");
        }
    }
}
