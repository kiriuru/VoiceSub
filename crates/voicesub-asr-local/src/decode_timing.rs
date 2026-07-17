//! P0 decode timing — measure VoiceSub stages vs Parakeet black-box wall time.
//!
//! Stages we own:
//! - `prepare` — cumulative buffer copy / window prep
//! - `preprocess` — gain / preemphasis / noise gate
//!
//! Inside `parakeet-rs::transcribe_samples` (not split without a VsInference wrapper):
//! - mel / features
//! - OrtValue::from_array
//! - Session::run (encoder + decoder_joint)
//! - extract + token decode
//!
//! Compare `outside_*` vs `parakeet_transcribe_us` to decide whether buffer reuse / IOBinding
//! is worth pursuing before pipeline window policy.

use serde::Serialize;

use crate::capture::PARAKEET_SAMPLE_RATE;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodeTimingBreakdown {
    pub audio_samples: u64,
    pub audio_ms: u32,
    /// Cumulative → decode window copy / selection.
    pub prepare_us: u64,
    /// Mic preprocess (gain / preemphasis / gate).
    pub preprocess_us: u64,
    /// Full `ParakeetTDT::transcribe_samples` wall (mel + ORT + tokenize).
    pub parakeet_transcribe_us: u64,
    /// prepare + preprocess + parakeet.
    pub total_us: u64,
    /// prepare + preprocess.
    pub outside_us: u64,
    /// `outside_us / total_us * 100` (0 if total is 0).
    pub outside_pct: f64,
    /// `parakeet_transcribe_us / total_us * 100`.
    pub parakeet_pct: f64,
}

impl DecodeTimingBreakdown {
    pub fn from_parts(
        audio_samples: usize,
        prepare_us: u64,
        preprocess_us: u64,
        parakeet_transcribe_us: u64,
    ) -> Self {
        let total_us = prepare_us
            .saturating_add(preprocess_us)
            .saturating_add(parakeet_transcribe_us);
        let outside_us = prepare_us.saturating_add(preprocess_us);
        let (outside_pct, parakeet_pct) = pct_pair(outside_us, parakeet_transcribe_us, total_us);
        Self {
            audio_samples: audio_samples as u64,
            audio_ms: samples_to_ms(audio_samples),
            prepare_us,
            preprocess_us,
            parakeet_transcribe_us,
            total_us,
            outside_us,
            outside_pct,
            parakeet_pct,
        }
    }

    pub fn total_ms(&self) -> u64 {
        self.total_us / 1000
    }
}

fn samples_to_ms(samples: usize) -> u32 {
    ((samples as u64 * 1000) / u64::from(PARAKEET_SAMPLE_RATE)) as u32
}

fn pct_pair(outside_us: u64, parakeet_us: u64, total_us: u64) -> (f64, f64) {
    if total_us == 0 {
        return (0.0, 0.0);
    }
    let total = total_us as f64;
    (
        (outside_us as f64) * 100.0 / total,
        (parakeet_us as f64) * 100.0 / total,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_pct_when_parakeet_dominates() {
        let t = DecodeTimingBreakdown::from_parts(16_000, 500, 500, 99_000);
        assert_eq!(t.outside_us, 1_000);
        assert_eq!(t.total_us, 100_000);
        assert!((t.outside_pct - 1.0).abs() < 0.01);
        assert!((t.parakeet_pct - 99.0).abs() < 0.01);
        assert_eq!(t.audio_ms, 1000);
    }

    #[test]
    fn zero_total_yields_zero_pct() {
        let t = DecodeTimingBreakdown::from_parts(0, 0, 0, 0);
        assert_eq!(t.outside_pct, 0.0);
        assert_eq!(t.parakeet_pct, 0.0);
    }
}
