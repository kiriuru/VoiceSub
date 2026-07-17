//! SST `segment_audio_enqueue` — delta audio queue before ASR decode.

use std::time::{Duration, Instant};

/// Tracks how much new audio arrived since the last decode and whether a timer elapsed.
#[derive(Debug, Clone)]
pub struct SegmentAudioEnqueue {
    delta_samples: usize,
    delta_threshold_samples: usize,
    decode_interval: Duration,
    last_decode: Instant,
    min_buffer_samples: usize,
}

impl SegmentAudioEnqueue {
    pub fn new(
        delta_ms: u32,
        decode_interval_ms: u32,
        sample_rate: u32,
        min_buffer_ms: u32,
    ) -> Self {
        Self {
            delta_samples: 0,
            delta_threshold_samples: ms_to_samples(delta_ms, sample_rate),
            decode_interval: Duration::from_millis(decode_interval_ms as u64),
            last_decode: Instant::now(),
            min_buffer_samples: ms_to_samples(min_buffer_ms, sample_rate),
        }
    }

    pub fn reset(&mut self) {
        self.delta_samples = 0;
        self.last_decode = Instant::now();
    }

    pub fn clear_after_decode(&mut self) {
        self.delta_samples = 0;
        self.last_decode = Instant::now();
    }

    pub fn push_samples(&mut self, count: usize) {
        self.delta_samples = self.delta_samples.saturating_add(count);
    }

    pub fn delta_samples(&self) -> usize {
        self.delta_samples
    }

    pub fn enough_new_audio(&self) -> bool {
        self.delta_samples >= self.delta_threshold_samples
    }

    pub fn decode_due(&self) -> bool {
        self.last_decode.elapsed() >= self.decode_interval
    }

    /// Returns true when decode should run given buffer length and speech hint.
    pub fn should_decode(&self, buffer_samples: usize, speech_active: bool) -> bool {
        if buffer_samples < self.min_buffer_samples {
            return false;
        }
        self.decode_due() && (speech_active || self.enough_new_audio())
    }
}

pub fn ms_to_samples(ms: u32, sample_rate: u32) -> usize {
    (sample_rate as u64 * ms as u64 / 1000) as usize
}

/// SST `slice_segment_audio_delta` — return only suffix not yet enqueued.
pub fn slice_segment_audio_delta(
    segment_audio: &[f32],
    segment_id: &str,
    started_now: bool,
    queued_sample_len_by_segment: &mut std::collections::HashMap<String, usize>,
) -> (Vec<f32>, bool) {
    let key = segment_id.trim();
    if key.is_empty() {
        return (segment_audio.to_vec(), segment_audio.is_empty());
    }

    if started_now {
        queued_sample_len_by_segment.insert(key.to_string(), 0);
    }

    let mut previous_len = queued_sample_len_by_segment
        .get(key)
        .copied()
        .unwrap_or(0);
    let total_len = segment_audio.len();
    if previous_len > total_len {
        queued_sample_len_by_segment.insert(key.to_string(), 0);
        previous_len = 0;
    }
    let delta = segment_audio[previous_len..].to_vec();
    let skip = delta.is_empty();
    queued_sample_len_by_segment.insert(key.to_string(), total_len);
    (delta, skip)
}

pub fn clear_segment_audio_enqueue_state(
    queued_sample_len_by_segment: &mut std::collections::HashMap<String, usize>,
    segment_id: Option<&str>,
) {
    match segment_id.map(str::trim).filter(|id| !id.is_empty()) {
        Some(key) => {
            queued_sample_len_by_segment.remove(key);
        }
        None => queued_sample_len_by_segment.clear(),
    }
}

/// SST parity: allow decode on very short trailing segments (~50 ms).
pub const DEFAULT_MIN_DECODE_BUFFER_MS: u32 = 50;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_delta_threshold() {
        let mut queue = SegmentAudioEnqueue::new(280, 400, 16_000, 200);
        queue.push_samples(1000);
        assert!(!queue.enough_new_audio());
        queue.push_samples(4000);
        assert!(queue.enough_new_audio());
    }

    #[test]
    fn should_decode_requires_min_buffer() {
        let queue = SegmentAudioEnqueue::new(280, 400, 16_000, 200);
        assert!(!queue.should_decode(1000, true));
    }

    #[test]
    fn slice_delta_rewinds_when_tracker_ahead_of_shorter_clip() {
        let mut queued = std::collections::HashMap::new();
        let partial_buffer = vec![0.0; 16_000];
        let (_, _) = slice_segment_audio_delta(&partial_buffer, "segment-1", true, &mut queued);

        let shorter_vad_clip = vec![0.0; 12_000];
        let (final_delta, final_skip) =
            slice_segment_audio_delta(&shorter_vad_clip, "segment-1", false, &mut queued);
        assert!(!final_skip);
        assert_eq!(final_delta.len(), shorter_vad_clip.len());
    }
}
