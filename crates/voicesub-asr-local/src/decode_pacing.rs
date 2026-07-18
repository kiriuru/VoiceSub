//! P1 decode pacing — stretch partial decode cadence as the cumulative buffer grows.
//!
//! Parakeet TDT re-decodes the full segment each partial; encoder time scales with
//! audio length. Fixed 200–280 ms intervals waste ORT once the phrase is multi-second.
//! Finals are never paced.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Shared last successful decode wall time (ms) — written by decode worker, read by pacer.
#[derive(Debug, Default)]
pub struct LastDecodeMs(AtomicU64);

impl LastDecodeMs {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(AtomicU64::new(0)))
    }

    pub fn store(&self, ms: u64) {
        self.0.store(ms, Ordering::Relaxed);
    }

    pub fn load(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// Effective partial decode interval for the current buffer / last decode cost.
///
/// - First ~1 s of speech stays near the preset base (first-paint).
/// - 1–5 s stretches toward ~3× base.
/// - Beyond ~5 s (long Live phrases) keeps stretching toward ~5×, capped at 3 s,
///   so encoder cost does not schedule a full redecode every 200–280 ms.
/// - If the last decode wall exceeds the stretched interval, wait at least 1.25× that cost.
pub fn adaptive_partial_decode_interval_ms(
    base_ms: u32,
    audio_ms: u32,
    last_decode_ms: u64,
) -> u32 {
    let base = base_ms.max(1);
    let stretch = if audio_ms <= 1_000 {
        1.0
    } else if audio_ms <= 5_000 {
        let over = f64::from(audio_ms.saturating_sub(1_000));
        (1.0 + over / 2_000.0).min(3.0)
    } else {
        let over = f64::from(audio_ms.saturating_sub(5_000));
        (3.0 + over / 5_000.0).min(5.0)
    };
    let mut interval = (f64::from(base) * stretch).round() as u32;
    if last_decode_ms > 0 {
        let floor = ((last_decode_ms as f64) * 1.25).round() as u32;
        interval = interval.max(floor);
    }
    interval.clamp(base, 3_000)
}

#[derive(Debug)]
pub struct DecodePacer {
    last_enqueue: Instant,
    /// Last interval that gated a successful enqueue (diagnostics).
    pub last_interval_ms: u32,
}

impl Default for DecodePacer {
    fn default() -> Self {
        Self {
            // Allow the first partial of a segment immediately (VAD already waited).
            last_enqueue: Instant::now()
                .checked_sub(Duration::from_secs(60))
                .unwrap_or_else(Instant::now),
            last_interval_ms: 0,
        }
    }
}

impl DecodePacer {
    /// Reset so the next partial is allowed immediately (new VAD segment).
    pub fn reset_for_new_segment(&mut self) {
        *self = Self::default();
    }

    /// Test helper: force the next `allow_partial` to pass.
    #[cfg(test)]
    pub fn allow_next(&mut self) {
        self.last_enqueue = Instant::now()
            .checked_sub(Duration::from_secs(60))
            .unwrap_or_else(Instant::now);
    }

    pub fn allow_partial(&mut self, interval_ms: u32) -> bool {
        let interval = Duration::from_millis(u64::from(interval_ms.max(1)));
        if self.last_enqueue.elapsed() < interval {
            return false;
        }
        self.last_interval_ms = interval_ms;
        self.last_enqueue = Instant::now();
        true
    }
}

/// Live VAD force-final ceiling (ms) when silence hold never fires.
///
/// Matches Local ASR UI / SST defaults (`maxSegmentMs: 5500`). Silence
/// (`min_silence_ms`) remains the primary finalize path; this ceiling stops
/// sticky WebRTC VAD from growing one partial for minutes without a Final.
pub fn max_segment_ms_for_preset(_preset: &str) -> u32 {
    5_500
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_audio_keeps_base_interval() {
        assert_eq!(adaptive_partial_decode_interval_ms(280, 800, 0), 280);
        assert_eq!(adaptive_partial_decode_interval_ms(200, 1_000, 0), 200);
    }

    #[test]
    fn longer_audio_stretches_interval() {
        let at_3s = adaptive_partial_decode_interval_ms(280, 3_000, 0);
        assert_eq!(at_3s, 560); // 280 * (1 + 2000/2000) = 560
        let at_5s = adaptive_partial_decode_interval_ms(280, 5_000, 0);
        assert_eq!(at_5s, 840); // 280 * 3.0
        let at_15s = adaptive_partial_decode_interval_ms(280, 15_000, 0);
        assert_eq!(at_15s, 1_400); // 280 * 5.0
    }

    #[test]
    fn last_decode_cost_raises_floor() {
        // 250 ms decode → floor 312.5 ≈ 313; stretch at 800 ms audio is still base 280
        let interval = adaptive_partial_decode_interval_ms(280, 800, 250);
        assert_eq!(interval, 313);
    }

    #[test]
    fn pacer_blocks_until_interval_elapses() {
        let mut pacer = DecodePacer::default();
        assert!(pacer.allow_partial(50));
        assert!(!pacer.allow_partial(50));
        std::thread::sleep(Duration::from_millis(60));
        assert!(pacer.allow_partial(50));
    }

    #[test]
    fn preset_max_segment_force_finals_like_ui() {
        assert_eq!(max_segment_ms_for_preset("low"), 5_500);
        assert_eq!(max_segment_ms_for_preset("balanced"), 5_500);
        assert_eq!(max_segment_ms_for_preset("quality"), 5_500);
    }
}
