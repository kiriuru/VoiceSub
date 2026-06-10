use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Value};
use voicesub_logging::{subtitle_trace, StructuredRuntimeLogger};

use crate::types::{LifecycleState, SubtitlePayloadEvent, TranscriptEvent, TranslationEvent};

pub type StructuredLogFn = Arc<dyn Fn(&str, &str, Value) + Send + Sync>;

fn subtitle_source_for_channel(channel: &str) -> &'static str {
    match channel {
        "subtitle_lifecycle" => "subtitle_lifecycle_core",
        "subtitle_presentation" => "subtitle_presentation",
        "subtitle_router" => "subtitle_router",
        "subtitle_overlay" => "overlay_broadcaster",
        _ => "subtitle",
    }
}

pub fn structured_log_from_runtime_logger(
    logger: Arc<StructuredRuntimeLogger>,
) -> StructuredLogFn {
    Arc::new(move |channel, event, fields| {
        let mut map = BTreeMap::new();
        if let Some(obj) = fields.as_object() {
            for (key, value) in obj {
                map.insert(key.clone(), value.clone());
            }
        }
        logger.log(
            channel,
            event,
            Some(subtitle_source_for_channel(channel)),
            Some(map),
        );
    })
}

#[derive(Clone, Default)]
pub struct SubtitleLog {
    structured: Option<StructuredLogFn>,
}

impl SubtitleLog {
    pub fn new(structured: Option<StructuredLogFn>) -> Self {
        Self { structured }
    }

    fn emit(&self, lane: &str, source: &str, event: &str, fields: Value) {
        if let Some(ref logger) = self.structured {
            logger(lane, event, fields.clone());
        }
        subtitle_trace(lane, source, event, fields);
    }

    pub(crate) fn transcript_partial(&self, event: &TranscriptEvent) {
        self.emit(
            "subtitle_lifecycle",
            "subtitle_lifecycle_core",
            "transcript_partial",
            json!({
                "sequence": event.sequence,
                "text_len": event.text.chars().count(),
            }),
        );
    }

    pub(crate) fn transcript_final(&self, event: &TranscriptEvent) {
        self.emit(
            "subtitle_lifecycle",
            "subtitle_lifecycle_core",
            "transcript_final",
            json!({
                "sequence": event.sequence,
                "text_len": event.text.chars().count(),
            }),
        );
    }

    pub(crate) fn translation_received(&self, event: &TranslationEvent) {
        let lines: Vec<Value> = event
            .translations
            .iter()
            .map(|item| {
                json!({
                    "slot_id": item.slot_id,
                    "target_lang": item.target_lang,
                    "text_len": item.text.chars().count(),
                    "success": item.success,
                })
            })
            .collect();
        self.emit(
            "subtitle_lifecycle",
            "subtitle_lifecycle_core",
            "translation_received",
            json!({
                "sequence": event.sequence,
                "source_text_len": event.source_text.chars().count(),
                "is_complete": event.is_complete,
                "translations": lines,
            }),
        );
    }

    pub(crate) fn promote_or_defer(
        &self,
        sequence: u64,
        outcome: &str,
        reason: &str,
        extra: Value,
    ) {
        self.emit(
            "subtitle_lifecycle",
            "subtitle_lifecycle_core",
            "promote_or_defer",
            json!({
                "sequence": sequence,
                "outcome": outcome,
                "reason": reason,
                "extra": extra,
            }),
        );
    }

    pub(crate) fn ttl_schedule(&self, fields: Value) {
        self.emit(
            "subtitle_lifecycle",
            "subtitle_lifecycle_core",
            "ttl_schedule",
            fields,
        );
    }

    pub(crate) fn ttl_expiry_tick(&self, sequence: u64, outcome: &str, fields: Value) {
        self.emit(
            "subtitle_lifecycle",
            "subtitle_lifecycle_core",
            "ttl_expiry_tick",
            json!({
                "sequence": sequence,
                "outcome": outcome,
                "details": fields,
            }),
        );
    }

    pub(crate) fn completed_payload_hidden(&self, sequence: u64, hide_source: bool) {
        self.emit(
            "subtitle_lifecycle",
            "subtitle_lifecycle_core",
            "completed_payload_hidden",
            json!({
                "sequence": sequence,
                "hide_source": hide_source,
            }),
        );
    }

    pub(crate) fn ttl_visibility(
        &self,
        sequence: u64,
        source_visible: bool,
        translation_visible: bool,
    ) {
        self.emit(
            "subtitle_lifecycle",
            "subtitle_lifecycle_core",
            "ttl_visibility",
            json!({
                "sequence": sequence,
                "source_visible": source_visible,
                "translation_visible": translation_visible,
            }),
        );
    }

    pub(crate) fn presentation_built(
        &self,
        lifecycle_state: LifecycleState,
        has_active_partial: bool,
        has_completed_translation: bool,
        payload: &SubtitlePayloadEvent,
    ) {
        self.emit(
            "subtitle_presentation",
            "subtitle_presentation",
            "presentation_built",
            json!({
                "lifecycle_state": lifecycle_state_str(lifecycle_state),
                "has_active_partial": has_active_partial,
                "has_completed_translation": has_completed_translation,
                "payload": payload_summary(payload),
            }),
        );
    }

    pub(crate) fn payload_published(&self, payload: &SubtitlePayloadEvent) {
        self.emit(
            "subtitle_router",
            "subtitle_router",
            "subtitle_payload_published",
            payload_summary(payload),
        );
    }

    pub(crate) fn overlay_publish(&self, published: bool, payload: &SubtitlePayloadEvent, reason: &str) {
        let event = if published {
            "overlay_update_published"
        } else {
            "overlay_update_deduped"
        };
        self.emit(
            "subtitle_overlay",
            "overlay_broadcaster",
            event,
            json!({
                "reason": reason,
                "payload": payload_summary(payload),
            }),
        );
    }

    pub(crate) fn lifecycle_reset(&self) {
        self.emit(
            "subtitle_lifecycle",
            "subtitle_lifecycle_core",
            "subtitle_reset",
            json!({}),
        );
    }

    pub(crate) fn lifecycle_config(&self, config: &Value) {
        self.emit(
            "subtitle_lifecycle",
            "subtitle_lifecycle_core",
            "lifecycle_config",
            config.clone(),
        );
    }

    pub(crate) fn stale_translation_suppressed(&self, sequence: u64) {
        self.emit(
            "subtitle_presentation",
            "subtitle_presentation",
            "overlay_stale_translation_suppressed",
            json!({ "sequence": sequence }),
        );
    }

    pub(crate) fn payload_mismatch(&self, sequence: u64) {
        self.emit(
            "subtitle_presentation",
            "subtitle_presentation",
            "overlay_payload_mismatch",
            json!({ "sequence": sequence }),
        );
    }
}

pub(crate) fn payload_summary(payload: &SubtitlePayloadEvent) -> Value {
    let visible_texts: Vec<&str> = payload
        .visible_items
        .iter()
        .map(|item| item.text.as_str())
        .filter(|text| !text.is_empty())
        .collect();
    json!({
        "sequence": payload.sequence,
        "lifecycle_state": lifecycle_state_str(payload.lifecycle_state),
        "completed_block_visible": payload.completed_block_visible,
        "active_partial_text": payload.active_partial_text,
        "active_partial_sequence": payload.active_partial_sequence,
        "visible_texts": visible_texts,
        "visible_kinds": payload.visible_items.iter().map(|item| item.kind.as_str()).collect::<Vec<_>>(),
        "show_source": payload.show_source,
        "show_translations": payload.show_translations,
        "completed_expires_at_utc": payload.completed_expires_at_utc,
        "line1": payload.line1,
        "line2": payload.line2,
    })
}

pub(crate) fn lifecycle_state_str(state: LifecycleState) -> &'static str {
    match state {
        LifecycleState::Idle => "idle",
        LifecycleState::PartialOnly => "partial_only",
        LifecycleState::CompletedOnly => "completed_only",
        LifecycleState::CompletedWithPartial => "completed_with_partial",
    }
}
