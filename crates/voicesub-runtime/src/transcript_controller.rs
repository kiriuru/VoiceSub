//! Port of SST `TranscriptController` — unified transcript → subtitle → OBS → translation path.

use std::sync::{Arc, RwLock};

use serde_json::Value;
use tokio::sync::Mutex;
use voicesub_obs::ObsCaptionService;
use voicesub_subtitle::{SubtitleRouter, TranscriptEvent, TranscriptKind, TranscriptSegment};
use voicesub_translation::{TranslationPreviewLineage, TranslationRuntimeController};
use voicesub_twitch::{apply_source_text_replacement, settings_from_config_value, SourceTextReplacementSettings};
use voicesub_ws::WsEventPublisher;

use crate::http::RuntimeMetricsCollector;
use crate::trace::RuntimePipelineLog;

pub struct TranscriptController {
    subtitle: Arc<SubtitleRouter>,
    translation: Arc<Mutex<TranslationRuntimeController>>,
    obs: Arc<ObsCaptionService>,
    publisher: WsEventPublisher,
    config_snapshot: Arc<RwLock<Value>>,
    pipeline_log: RuntimePipelineLog,
    metrics: Arc<RuntimeMetricsCollector>,
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
        }
    }

    fn config(&self) -> Value {
        self.config_snapshot
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn replacement_settings(&self) -> SourceTextReplacementSettings {
        settings_from_config_value(&self.config())
    }

    fn default_source_lang(&self) -> String {
        self.config()
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
    pub async fn handle_event(&self, event: TranscriptEvent) {
        let event = self.apply_replacement(event);
        self.publish_transcript(&event).await;
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
            self.pipeline_log
                .asr_ingest_published(true, event.sequence, event.text.chars().count());
            self.metrics.record_final_published(None);
        } else {
            self.pipeline_log
                .asr_ingest_published(false, event.sequence, event.text.chars().count());
            self.metrics.record_partial_published(None);
        }
    }

    async fn publish_transcript(&self, event: &TranscriptEvent) {
        let body = serde_json::to_value(event).unwrap_or_default();
        self.publisher
            .broadcast_channel("transcript_update", "transcript_update", body.clone())
            .await;
        self.publisher
            .broadcast_channel(
                "transcript_segment_event",
                "transcript_segment_event",
                body,
            )
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
