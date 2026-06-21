//! Port of SST `TranscriptController` — unified transcript → subtitle → OBS → translation path.

use std::sync::{Arc, Mutex as StdMutex, RwLock};
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::Mutex;
use voicesub_obs::ObsCaptionService;
use voicesub_subtitle::{SubtitleRouter, TranscriptEvent, TranscriptKind, TranscriptSegment};
use voicesub_translation::{TranslationPreviewLineage, TranslationRuntimeController};
use voicesub_twitch::{
    SourceTextReplacementSettings, apply_source_text_replacement, settings_from_section_value,
};
use voicesub_ws::WsEventPublisher;

use crate::http::RuntimeMetricsCollector;
use crate::trace::RuntimePipelineLog;

/// Minimum spacing between `transcript_update` partial broadcasts (review §2). Browser
/// Web Speech emits interim hypotheses many times per second; each one previously produced
/// a `transcript_update` IPC/WS event in addition to `overlay_update`. Coalescing partials
/// here caps that rate without touching the subtitle lifecycle or overlay path (those still
/// see every partial). Final transcripts always bypass the throttle.
const PARTIAL_TRANSCRIPT_MIN_INTERVAL_MS: u64 = 90;

fn partial_transcript_min_interval() -> Duration {
    let ms = std::env::var("VOICESUB_TRANSCRIPT_PARTIAL_MIN_INTERVAL_MS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .unwrap_or(PARTIAL_TRANSCRIPT_MIN_INTERVAL_MS);
    Duration::from_millis(ms)
}

/// Leading-edge throttle for partial `transcript_update` broadcasts. A new phrase
/// (changed `segment_id`) always emits immediately; repeat partials of the same phrase are
/// rate-limited even when `event.sequence` increments on every interim hypothesis.
/// The very latest partial of a burst may be coalesced away, but the matching
/// `overlay_update` still carries it and a final transcript follows shortly.
#[derive(Debug, Default)]
struct PartialTranscriptThrottle {
    interval: Duration,
    last_emit: Option<Instant>,
    last_phrase_key: Option<String>,
}

impl PartialTranscriptThrottle {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_emit: None,
            last_phrase_key: None,
        }
    }

    fn should_emit_partial(&mut self, phrase_key: &str, now: Instant) -> bool {
        let new_phrase = self.last_phrase_key.as_deref() != Some(phrase_key);
        let due = self
            .last_emit
            .map(|previous| now.duration_since(previous) >= self.interval)
            .unwrap_or(true);
        if new_phrase || due {
            self.last_phrase_key = Some(phrase_key.to_string());
            self.last_emit = Some(now);
            return true;
        }
        false
    }

    fn note_final(&mut self) {
        self.last_emit = None;
        self.last_phrase_key = None;
    }
}

pub struct TranscriptController {
    subtitle: Arc<SubtitleRouter>,
    translation: Arc<Mutex<TranslationRuntimeController>>,
    obs: Arc<ObsCaptionService>,
    publisher: WsEventPublisher,
    config_snapshot: Arc<RwLock<Value>>,
    pipeline_log: RuntimePipelineLog,
    metrics: Arc<RuntimeMetricsCollector>,
    partial_throttle: StdMutex<PartialTranscriptThrottle>,
}

impl TranscriptController {
    pub fn new(
        subtitle: Arc<SubtitleRouter>,
        translation: Arc<Mutex<TranslationRuntimeController>>,
        obs: Arc<ObsCaptionService>,
        publisher: WsEventPublisher,
        config_snapshot: Arc<RwLock<Value>>,
        pipeline_log: RuntimePipelineLog,
        metrics: Arc<RuntimeMetricsCollector>,
    ) -> Self {
        Self {
            subtitle,
            translation,
            obs,
            publisher,
            config_snapshot,
            pipeline_log,
            metrics,
            partial_throttle: StdMutex::new(PartialTranscriptThrottle::new(
                partial_transcript_min_interval(),
            )),
        }
    }

    fn replacement_settings(&self) -> SourceTextReplacementSettings {
        self.config_snapshot
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get("source_text_replacement")
            .map(settings_from_section_value)
            .unwrap_or_default()
    }

    fn default_source_lang(&self) -> String {
        self.config_snapshot
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get("source_lang")
            .and_then(|v| v.as_str())
            .unwrap_or("auto")
            .trim()
            .to_ascii_lowercase()
    }

    fn apply_replacement(&self, mut event: TranscriptEvent) -> TranscriptEvent {
        let settings = self.replacement_settings();
        let routed = apply_source_text_replacement(&event.text, &settings);
        event.text = routed.clone();
        if let Some(segment) = event.segment.as_mut() {
            segment.text = routed;
        }
        event
    }

    fn event_source_lang(&self, event: &TranscriptEvent) -> String {
        event
            .segment
            .as_ref()
            .map(|segment| segment.source_lang.as_str())
            .unwrap_or(self.default_source_lang().as_str())
            .trim()
            .to_ascii_lowercase()
    }

    /// SST parity: `backend/core/runtime/transcript_controller.py::handle_event`
    /// — subtitle record must exist before `submit_final` (dispatcher relevance).
    pub async fn handle_event(&self, event: TranscriptEvent, ingest_started: Option<Instant>) {
        let ingest_latency_ms = ingest_started.map(|started| {
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            (ms * 10.0).round() / 10.0
        });
        let event = self.apply_replacement(event);
        // Only the redundant `transcript_update` WS/IPC channel is throttled here; the
        // subtitle lifecycle and OBS source path below still receive every partial (§2).
        if self.should_publish_transcript(&event) {
            self.publish_transcript(&event).await;
        }
        self.subtitle.handle_transcript(event.clone()).await;
        self.publish_source_event(&event).await;

        if event.event == TranscriptKind::Final {
            let source_lang = self.event_source_lang(&event);
            let preview_lineage_key = preview_lineage_key_from_segment(event.segment.as_ref());
            let mut controller = self.translation.lock().await;
            controller
                .submit_final(
                    event.sequence,
                    &event.text,
                    &source_lang,
                    preview_lineage_key.as_deref(),
                )
                .await;
            self.pipeline_log.asr_ingest_published(
                true,
                event.sequence,
                event.text.chars().count(),
                ingest_latency_ms,
            );
            self.metrics.record_final_published(ingest_latency_ms);
        } else {
            self.pipeline_log.asr_ingest_published(
                false,
                event.sequence,
                event.text.chars().count(),
                ingest_latency_ms,
            );
            self.metrics.record_partial_published(ingest_latency_ms);
        }
    }

    fn should_publish_transcript(&self, event: &TranscriptEvent) -> bool {
        let mut throttle = self
            .partial_throttle
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if event.event == TranscriptKind::Final {
            throttle.note_final();
            return true;
        }
        let Some(phrase_key) = event
            .segment
            .as_ref()
            .map(|segment| segment.segment_id.as_str())
        else {
            return true;
        };
        throttle.should_emit_partial(phrase_key, Instant::now())
    }

    async fn publish_transcript(&self, event: &TranscriptEvent) {
        let body = serde_json::to_value(event).unwrap_or_default();
        self.publisher
            .broadcast_channel("transcript_update", "transcript_update", body)
            .await;
    }

    async fn publish_source_event(&self, event: &TranscriptEvent) {
        let is_final = event.event == TranscriptKind::Final;
        self.obs.publish_source(&event.text, is_final);
    }
}

pub fn preview_lineage_key_from_segment(segment: Option<&TranscriptSegment>) -> Option<String> {
    let segment = segment?;
    TranslationPreviewLineage::lineage_key(Some(&segment.segment_id), Some(segment.revision))
}

#[cfg(test)]
mod tests {
    use super::*;
    use voicesub_subtitle::TranscriptKind;

    #[test]
    fn preview_lineage_uses_segment_id_and_revision() {
        let segment = TranscriptSegment {
            segment_id: "worker-g0-s1".into(),
            text: "hi".into(),
            is_final: true,
            source_lang: "en".into(),
            provider: Some("browser_google".into()),
            sequence: 1,
            revision: 3,
            start_ms: None,
            end_ms: None,
        };
        assert_eq!(
            preview_lineage_key_from_segment(Some(&segment)),
            Some("worker-g0-s1:3".into())
        );
    }

    #[test]
    fn partial_throttle_emits_first_and_rate_limits_repeats() {
        let mut throttle = PartialTranscriptThrottle::new(Duration::from_millis(90));
        let t0 = Instant::now();
        assert!(throttle.should_emit_partial("worker-g0-s1", t0));
        assert!(!throttle.should_emit_partial("worker-g0-s1", t0 + Duration::from_millis(30)));
        assert!(throttle.should_emit_partial("worker-g0-s1", t0 + Duration::from_millis(100)));
    }

    #[test]
    fn partial_throttle_new_phrase_always_emits() {
        let mut throttle = PartialTranscriptThrottle::new(Duration::from_millis(90));
        let t0 = Instant::now();
        assert!(throttle.should_emit_partial("worker-g0-s1", t0));
        assert!(throttle.should_emit_partial("worker-g0-s2", t0 + Duration::from_millis(5)));
    }

    #[test]
    fn partial_throttle_rate_limits_same_segment_despite_increasing_sequence() {
        let mut throttle = PartialTranscriptThrottle::new(Duration::from_millis(90));
        let t0 = Instant::now();
        assert!(throttle.should_emit_partial("worker-g0-s1", t0));
        assert!(!throttle.should_emit_partial("worker-g0-s1", t0 + Duration::from_millis(20)));
        assert!(!throttle.should_emit_partial("worker-g0-s1", t0 + Duration::from_millis(40)));
        assert!(throttle.should_emit_partial("worker-g0-s1", t0 + Duration::from_millis(100)));
    }

    #[test]
    fn partial_throttle_resets_after_final() {
        let mut throttle = PartialTranscriptThrottle::new(Duration::from_millis(90));
        let t0 = Instant::now();
        assert!(throttle.should_emit_partial("worker-g0-s7", t0));
        throttle.note_final();
        assert!(throttle.should_emit_partial("worker-g0-s7", t0 + Duration::from_millis(1)));
    }

    #[test]
    fn final_event_kind_matches_sst_contract() {
        let event = TranscriptEvent {
            event: TranscriptKind::Final,
            text: "hello".into(),
            sequence: 1,
            segment: None,
        };
        assert_eq!(event.event, TranscriptKind::Final);
    }
}
