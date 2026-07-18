//! Live emit counters for Local ASR diagnostics (§4.8 C4).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::pipeline::PipelineEmit;
use voicesub_partial_emit::{normalize_transcript_text, split_words};

/// Shared counters updated on the runtime / test emit path.
#[derive(Debug, Default)]
pub struct EmitTelemetry {
    pub partial_emits: AtomicU64,
    pub final_emits: AtomicU64,
    /// Partials that revised text without adding words (same length, different tokens).
    pub revision_emits: AtomicU64,
    /// Last observed speech→first-partial latency (ms) for a segment (`duration_ms` at first emit).
    pub last_first_partial_ms: AtomicU64,
    /// Last observed speech→final latency (ms) (`duration_ms` on Final).
    pub last_final_ms: AtomicU64,
    had_partial_this_segment: AtomicU64,
}

impl EmitTelemetry {
    pub fn new_shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn reset(&self) {
        self.partial_emits.store(0, Ordering::SeqCst);
        self.final_emits.store(0, Ordering::SeqCst);
        self.revision_emits.store(0, Ordering::SeqCst);
        self.last_first_partial_ms.store(0, Ordering::SeqCst);
        self.last_final_ms.store(0, Ordering::SeqCst);
        self.had_partial_this_segment.store(0, Ordering::SeqCst);
    }

    pub fn snapshot(&self) -> EmitTelemetrySnapshot {
        let partial = self.partial_emits.load(Ordering::SeqCst);
        let revision = self.revision_emits.load(Ordering::SeqCst);
        let revision_rate = if partial == 0 {
            0.0
        } else {
            revision as f64 / partial as f64
        };
        EmitTelemetrySnapshot {
            partial_emits: partial,
            final_emits: self.final_emits.load(Ordering::SeqCst),
            revision_emits: revision,
            revision_rate,
            last_first_partial_ms: nonzero_u32(self.last_first_partial_ms.load(Ordering::SeqCst)),
            last_final_ms: nonzero_u32(self.last_final_ms.load(Ordering::SeqCst)),
        }
    }

    pub fn record_emit(&self, emit: &PipelineEmit, previous_partial: &str, segment_speech_ms: u32) {
        if emit.is_final {
            self.final_emits.fetch_add(1, Ordering::SeqCst);
            if segment_speech_ms > 0 {
                self.last_final_ms
                    .store(u64::from(segment_speech_ms), Ordering::SeqCst);
            }
            self.had_partial_this_segment.store(0, Ordering::SeqCst);
            return;
        }

        self.partial_emits.fetch_add(1, Ordering::SeqCst);
        if self.had_partial_this_segment.swap(1, Ordering::SeqCst) == 0 && segment_speech_ms > 0 {
            self.last_first_partial_ms
                .store(u64::from(segment_speech_ms), Ordering::SeqCst);
        }
        if is_revision_without_growth(previous_partial, &emit.text) {
            self.revision_emits.fetch_add(1, Ordering::SeqCst);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmitTelemetrySnapshot {
    pub partial_emits: u64,
    pub final_emits: u64,
    pub revision_emits: u64,
    pub revision_rate: f64,
    pub last_first_partial_ms: Option<u32>,
    pub last_final_ms: Option<u32>,
}

fn nonzero_u32(value: u64) -> Option<u32> {
    if value == 0 {
        None
    } else {
        Some(value.min(u64::from(u32::MAX)) as u32)
    }
}

fn is_revision_without_growth(previous: &str, next: &str) -> bool {
    let prev = split_words(&normalize_transcript_text(previous));
    let new = split_words(&normalize_transcript_text(next));
    !prev.is_empty() && prev.len() == new.len() && prev != new
}

#[cfg(test)]
mod tests {
    use super::*;

    fn partial(text: &str) -> PipelineEmit {
        PipelineEmit {
            segment_id: "s1".into(),
            revision: 1,
            text: text.into(),
            is_final: false,
            is_speech: true,
        }
    }

    #[test]
    fn tracks_first_partial_and_revision_rate() {
        let tel = EmitTelemetry::default();
        tel.record_emit(&partial("hello"), "", 180);
        tel.record_emit(&partial("hallo"), "hello", 280);
        let snap = tel.snapshot();
        assert_eq!(snap.partial_emits, 2);
        assert_eq!(snap.revision_emits, 1);
        assert_eq!(snap.last_first_partial_ms, Some(180));
        assert!((snap.revision_rate - 0.5).abs() < f64::EPSILON);
    }
}
