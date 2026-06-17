//! Builds browser worker `TranscriptEvent`s (SST `runtime_orchestrator_browser_worker_mixin` port).
//!
//! Browser Web Speech already emits curated partial/final hypotheses from the worker.
//! `PartialEmitCoordinator::should_emit` (word_growth / char_delta) is for local Parakeet
//! streaming only — browser partials pass through without that filter.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::sync::Mutex;
use voicesub_subtitle::{TranscriptEvent, TranscriptKind, TranscriptSegment};

use crate::http::PartialEmitCoordinator;

const BROWSER_PROVIDER: &str = "browser_google";

pub struct BrowserTranscriptEventBuilder {
    runtime_running: Arc<AtomicBool>,
    partial_emit: Arc<Mutex<PartialEmitCoordinator>>,
}

impl BrowserTranscriptEventBuilder {
    pub fn new(
        runtime_running: Arc<AtomicBool>,
        partial_emit: Arc<Mutex<PartialEmitCoordinator>>,
    ) -> Self {
        Self {
            runtime_running,
            partial_emit,
        }
    }

    fn is_active(&self) -> bool {
        self.runtime_running.load(Ordering::Relaxed)
    }

    pub async fn build_partial_event(
        &self,
        partial_text: &str,
        source_lang: &str,
        client_segment_id: Option<&str>,
        _config: &serde_json::Value,
    ) -> Option<TranscriptEvent> {
        if !self.is_active() || partial_text.trim().is_empty() {
            return None;
        }

        let mut partial_emit = self.partial_emit.lock().await;
        let (segment_id, revision, _started_now, previous_to_clear) = partial_emit
            .segment_state
            .assign_segment_tracking(client_segment_id);
        if let Some(previous) = previous_to_clear.as_deref() {
            partial_emit
                .segment_state
                .clear_partial_tracking_for_segment(Some(previous));
        }

        let sequence = partial_emit.segment_state.next_sequence();
        let segment = build_worker_segment(
            &segment_id,
            revision,
            partial_text,
            false,
            source_lang,
            sequence,
        );
        Some(TranscriptEvent {
            event: TranscriptKind::Partial,
            text: partial_text.to_string(),
            sequence,
            segment: Some(segment),
        })
    }

    pub async fn build_final_event(
        &self,
        final_text: &str,
        source_lang: &str,
        client_segment_id: Option<&str>,
    ) -> Option<TranscriptEvent> {
        if !self.is_active() || final_text.trim().is_empty() {
            return None;
        }

        let mut partial_emit = self.partial_emit.lock().await;
        let (segment_id, revision, _started_now, previous_to_clear) = partial_emit
            .segment_state
            .assign_segment_tracking(client_segment_id);
        if let Some(previous) = previous_to_clear.as_deref() {
            partial_emit
                .segment_state
                .clear_partial_tracking_for_segment(Some(previous));
        }
        partial_emit.clear_segment(&segment_id);

        let sequence = partial_emit.segment_state.next_sequence();
        let segment = build_worker_segment(
            &segment_id,
            revision,
            final_text,
            true,
            source_lang,
            sequence,
        );
        partial_emit.segment_state.clear_active_segment();

        Some(TranscriptEvent {
            event: TranscriptKind::Final,
            text: final_text.to_string(),
            sequence,
            segment: Some(segment),
        })
    }
}

fn build_worker_segment(
    segment_id: &str,
    revision: u64,
    text: &str,
    is_final: bool,
    source_lang: &str,
    sequence: u64,
) -> TranscriptSegment {
    TranscriptSegment {
        segment_id: segment_id.to_string(),
        text: text.to_string(),
        is_final,
        source_lang: source_lang.to_string(),
        provider: Some(BROWSER_PROVIDER.into()),
        sequence,
        revision,
        start_ms: Some(0),
        end_ms: Some(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use voicesub_subtitle::TranscriptKind;

    fn builder() -> BrowserTranscriptEventBuilder {
        BrowserTranscriptEventBuilder::new(
            Arc::new(AtomicBool::new(true)),
            Arc::new(Mutex::new(PartialEmitCoordinator::default())),
        )
    }

    #[tokio::test]
    async fn browser_partial_passes_through_without_word_growth_filter() {
        let builder = builder();
        let config = serde_json::json!({
            "asr": {
                "realtime": {
                    "partial_emit_mode": "word_growth",
                    "partial_min_new_words": 2
                }
            }
        });
        let first = builder
            .build_partial_event("hello", "en", Some("seg-1"), &config)
            .await
            .expect("first partial");
        let second = builder
            .build_partial_event("hello", "en", Some("seg-1"), &config)
            .await
            .expect("repeat partial must not be suppressed for browser worker");
        assert_eq!(first.event, TranscriptKind::Partial);
        assert_eq!(second.event, TranscriptKind::Partial);
        assert_ne!(first.sequence, second.sequence);
    }

    #[tokio::test]
    async fn browser_partial_emits_char_growth_without_coalescing_gate() {
        let builder = builder();
        let config = serde_json::json!({ "asr": { "realtime": {} } });
        let first = builder
            .build_partial_event("hel", "en", Some("seg-2"), &config)
            .await
            .expect("char partial");
        let second = builder
            .build_partial_event("hello", "en", Some("seg-2"), &config)
            .await
            .expect("grown partial");
        assert_eq!(first.text, "hel");
        assert_eq!(second.text, "hello");
    }
}
