//! SST `local_asr_recognition_processing` — mic preprocess before Parakeet decode.

use crate::config::LocalAsrRecognitionConfig;

#[derive(Debug, Clone)]
pub struct RecognitionProcessor {
    config: LocalAsrRecognitionConfig,
}

impl RecognitionProcessor {
    pub fn new(config: LocalAsrRecognitionConfig) -> Self {
        Self { config }
    }

    pub fn reset(&mut self) {
        // Reserved for future streaming decode state; segment boundaries pass explicit prev.
    }

    /// In-place preprocess immediately before Parakeet decode.
    ///
    /// `preemphasis_prev_start` is the raw sample before `samples[0]` (0.0 at segment start).
    pub fn preprocess_for_decode(&self, samples: &mut [f32], preemphasis_prev_start: f32) {
        if samples.is_empty() {
            return;
        }
        let gain = self.config.input_gain.clamp(0.1, 4.0);
        if (gain - 1.0).abs() > f32::EPSILON {
            for sample in samples.iter_mut() {
                *sample *= gain;
            }
        }
        if self.config.preemphasis_enabled {
            let mut prev = preemphasis_prev_start;
            apply_preemphasis(
                samples,
                self.config.preemphasis_coeff.clamp(0.0, 0.99),
                &mut prev,
            );
        }
        if self.config.noise_gate_enabled {
            let threshold = self.config.noise_gate_threshold.max(0.0);
            for sample in samples.iter_mut() {
                if sample.abs() < threshold {
                    *sample = 0.0;
                }
            }
        }
    }

    /// Legacy helper used by golden tests — treats the buffer as a standalone segment.
    pub fn process_in_place(&self, samples: &mut [f32]) {
        self.preprocess_for_decode(samples, 0.0);
    }
}

impl From<&LocalAsrRecognitionConfig> for RecognitionProcessor {
    fn from(value: &LocalAsrRecognitionConfig) -> Self {
        Self::new(value.clone())
    }
}

pub fn preemphasis_prev_before_index(segment: &[f32], index: usize) -> f32 {
    if index == 0 { 0.0 } else { segment[index - 1] }
}

fn apply_preemphasis(samples: &mut [f32], coeff: f32, prev: &mut f32) {
    let mut last = *prev;
    for sample in samples.iter_mut() {
        let current = *sample;
        *sample = current - coeff * last;
        last = current;
    }
    *prev = last;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_scales_samples() {
        let processor = RecognitionProcessor::new(LocalAsrRecognitionConfig {
            input_gain: 2.0,
            ..LocalAsrRecognitionConfig::default()
        });
        let mut samples = vec![0.25, -0.5];
        processor.process_in_place(&mut samples);
        assert_eq!(samples, vec![0.5, -1.0]);
    }

    #[test]
    fn noise_gate_zeros_quiet_bins() {
        let processor = RecognitionProcessor::new(LocalAsrRecognitionConfig {
            noise_gate_enabled: true,
            noise_gate_threshold: 0.05,
            ..LocalAsrRecognitionConfig::default()
        });
        let mut samples = vec![0.01, 0.2];
        processor.process_in_place(&mut samples);
        assert_eq!(samples[0], 0.0);
        assert_eq!(samples[1], 0.2);
    }

    #[test]
    fn preemphasis_uses_explicit_start_prev() {
        let processor = RecognitionProcessor::new(LocalAsrRecognitionConfig {
            preemphasis_enabled: true,
            preemphasis_coeff: 0.5,
            ..LocalAsrRecognitionConfig::default()
        });
        let mut samples = vec![1.0, 0.0];
        processor.preprocess_for_decode(&mut samples, 1.0);
        assert_eq!(samples[0], 0.5);
        assert_eq!(samples[1], -0.5);
    }
}
