//! ASR decode window selection — partial vs final for batch TDT.
//!
//! Parakeet TDT has no native streaming decoder. Live partials must re-decode the
//! **full cumulative segment buffer** so hypotheses grow (`hello` → `hello world`).
//! Sliding-window-only decode is retained only as an optional long-segment helper
//! and must not be the live emit path.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodePass {
    Partial,
    Final,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedDecodeWindow {
    pub samples: Vec<f32>,
    pub preemphasis_prev_start: f32,
}

#[allow(dead_code)]
pub fn left_context_ms(chunk_window_ms: u32) -> u32 {
    chunk_window_ms.saturating_mul(3).clamp(1800, 6000)
}

/// Tail window retained for tests / optional long-segment fallback.
///
/// **Not** used by live `prepare_decode_window` — sliding-only decode drops early
/// words once speech exceeds `left_context + chunk` and makes partials "spin".
#[allow(dead_code)]
pub fn select_partial_window(
    cumulative: &[f32],
    chunk_window_ms: u32,
    sample_rate: u32,
) -> (usize, Vec<f32>) {
    let left = crate::segment_enqueue::ms_to_samples(left_context_ms(chunk_window_ms), sample_rate);
    let chunk = crate::segment_enqueue::ms_to_samples(chunk_window_ms.max(640), sample_rate);
    let window_len = left.saturating_add(chunk);
    if cumulative.len() <= window_len {
        return (0, cumulative.to_vec());
    }
    let start = cumulative.len().saturating_sub(window_len);
    (start, cumulative[start..].to_vec())
}

/// Prepare audio for a decode pass (live + test path).
///
/// Both Partial and Final re-transcribe the growing segment buffer so partial
/// text accumulates instead of rewriting the latest chunk only.
pub fn prepare_decode_window(
    cumulative: &[f32],
    _chunk_window_ms: u32,
    pass: DecodePass,
) -> PreparedDecodeWindow {
    let _ = pass;
    PreparedDecodeWindow {
        preemphasis_prev_start: 0.0,
        samples: cumulative.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::SAMPLE_RATE;
    use crate::recognition_processing::preemphasis_prev_before_index;

    fn prepare_partial_tail_window(
        cumulative: &[f32],
        chunk_window_ms: u32,
    ) -> PreparedDecodeWindow {
        let (start, samples) = select_partial_window(cumulative, chunk_window_ms, SAMPLE_RATE);
        PreparedDecodeWindow {
            preemphasis_prev_start: preemphasis_prev_before_index(cumulative, start),
            samples,
        }
    }

    #[test]
    fn partial_and_final_use_full_cumulative_buffer() {
        let cumulative = vec![0.5; 16_000 * 4];
        let partial = prepare_decode_window(&cumulative, 640, DecodePass::Partial);
        let final_pass = prepare_decode_window(&cumulative, 640, DecodePass::Final);
        assert_eq!(partial.samples.len(), cumulative.len());
        assert_eq!(final_pass.samples.len(), cumulative.len());
        assert_eq!(partial.preemphasis_prev_start, 0.0);
        assert_eq!(final_pass.preemphasis_prev_start, 0.0);
    }

    #[test]
    fn partial_tail_helper_keeps_preemph_prev() {
        let mut cumulative = vec![0.5; 16_000 * 4];
        let (start, _) = select_partial_window(&cumulative, 640, SAMPLE_RATE);
        cumulative[start.saturating_sub(1)] = 0.1;
        let prepared = prepare_partial_tail_window(&cumulative, 640);
        assert_eq!(prepared.preemphasis_prev_start, 0.1);
        assert!(prepared.samples.len() < cumulative.len());
    }

    #[test]
    fn partial_tail_is_bounded_for_long_segments() {
        let cumulative = vec![1.0; 16_000 * 10];
        let (start, window) = select_partial_window(&cumulative, 640, SAMPLE_RATE);
        assert!(window.len() < cumulative.len());
        assert_eq!(window.len(), cumulative.len() - start);
    }
}
