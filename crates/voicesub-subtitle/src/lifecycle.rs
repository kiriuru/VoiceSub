use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tokio::task::JoinHandle;

use crate::presentation::SubtitlePresentation;
use crate::trace::SubtitleLog;
use crate::types::{
    SubtitleLineItem, SubtitlePayloadEvent, TranscriptEvent, TranscriptKind, TranslationEvent,
};

/// Bound lifecycle record map size for multi-hour sessions.
const LIFECYCLE_RECORDS_MAX: usize = 512;

pub(crate) struct SubtitleLifecycleCore {
    config_getter: Box<dyn Fn() -> Value + Send + Sync>,
    records: HashMap<u64, Value>,
    active_partial: Option<Value>,
    completed_sequence: Option<u64>,
    latest_final_sequence: Option<u64>,
    pending_final_sequence: Option<u64>,
    completed_expires_at_utc: Option<String>,
    completed_expires_at_monotonic: Option<f64>,
    completed_source_expires_at_monotonic: Option<f64>,
    completed_translation_expires_at_monotonic: Option<f64>,
    expiry_task: Option<JoinHandle<()>>,
    start_instant: Instant,
    schedule_expiry: Arc<dyn Fn(u64, f64) + Send + Sync>,
    log: SubtitleLog,
}

impl SubtitleLifecycleCore {
    pub fn new(
        config_getter: Box<dyn Fn() -> Value + Send + Sync>,
        schedule_expiry: Arc<dyn Fn(u64, f64) + Send + Sync>,
        log: SubtitleLog,
    ) -> Self {
        Self {
            config_getter,
            records: HashMap::new(),
            active_partial: None,
            completed_sequence: None,
            latest_final_sequence: None,
            pending_final_sequence: None,
            completed_expires_at_utc: None,
            completed_expires_at_monotonic: None,
            completed_source_expires_at_monotonic: None,
            completed_translation_expires_at_monotonic: None,
            expiry_task: None,
            start_instant: Instant::now(),
            schedule_expiry,
            log,
        }
    }

    pub fn config(&self) -> Value {
        (self.config_getter)()
    }

    pub fn lifecycle_config(&self) -> Value {
        let config = self.config();
        let lifecycle = config
            .get("subtitle_lifecycle")
            .cloned()
            .unwrap_or(Value::Null);
        let completed_ttl_ms = lifecycle
            .get("completed_block_ttl_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(4500)
            .max(500);
        let source_ttl_ms = lifecycle
            .get("completed_source_ttl_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(completed_ttl_ms)
            .max(500);
        let translation_ttl_ms = lifecycle
            .get("completed_translation_ttl_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(completed_ttl_ms)
            .max(500);
        // Deprecated keys (trace/diagnostics only; not used by lifecycle FSM):
        // pause_to_finalize_ms / finalization_hold_ms — use asr.browser.force_finalization_timeout_ms.
        // hard_max_phrase_ms / max_segment_ms — legacy; no active replacement.
        let pause_to_finalize_ms = lifecycle
            .get("pause_to_finalize_ms")
            .and_then(|v| v.as_u64())
            .or_else(|| {
                config
                    .get("asr")
                    .and_then(|v| v.get("realtime"))
                    .and_then(|v| v.get("finalization_hold_ms"))
                    .and_then(|v| v.as_u64())
            })
            .unwrap_or(350)
            .max(120);
        let hard_max_phrase_ms = lifecycle
            .get("hard_max_phrase_ms")
            .and_then(|v| v.as_u64())
            .or_else(|| {
                config
                    .get("asr")
                    .and_then(|v| v.get("realtime"))
                    .and_then(|v| v.get("max_segment_ms"))
                    .and_then(|v| v.as_u64())
            })
            .unwrap_or(5500)
            .max(1000);
        json!({
            "completed_block_ttl_ms": source_ttl_ms.max(translation_ttl_ms),
            "completed_source_ttl_ms": source_ttl_ms,
            "completed_translation_ttl_ms": translation_ttl_ms,
            "pause_to_finalize_ms": pause_to_finalize_ms,
            "allow_early_replace_on_next_final": lifecycle.get("allow_early_replace_on_next_final").and_then(|v| v.as_bool()).unwrap_or(true),
            "sync_source_and_translation_expiry": lifecycle.get("sync_source_and_translation_expiry").and_then(|v| v.as_bool()).unwrap_or(true),
            "keep_completed_translation_during_active_partial": lifecycle.get("keep_completed_translation_during_active_partial").and_then(|v| v.as_bool()).unwrap_or(true),
            "hard_max_phrase_ms": hard_max_phrase_ms,
        })
    }

    pub fn active_partial(&self) -> Option<Value> {
        self.active_partial.clone()
    }

    pub fn completed_expires_at_utc(&self) -> Option<String> {
        self.completed_expires_at_utc.clone()
    }

    pub(crate) fn completed_sequence(&self) -> Option<u64> {
        self.completed_sequence
    }

    pub fn record_for_sequence(&self, sequence: u64) -> Option<Value> {
        self.records.get(&sequence).cloned()
    }

    pub fn reset(&mut self) {
        if let Some(task) = self.expiry_task.take() {
            task.abort();
        }
        self.records.clear();
        self.active_partial = None;
        self.completed_sequence = None;
        self.latest_final_sequence = None;
        self.pending_final_sequence = None;
        self.completed_expires_at_utc = None;
        self.completed_expires_at_monotonic = None;
        self.completed_source_expires_at_monotonic = None;
        self.completed_translation_expires_at_monotonic = None;
        self.log.lifecycle_reset();
    }

    pub fn clear_active_partial(&mut self) {
        self.active_partial = None;
    }

    pub fn handle_transcript(
        &mut self,
        event: TranscriptEvent,
        presentation: &SubtitlePresentation,
    ) {
        if event.event == TranscriptKind::Partial {
            let source_lang = event
                .segment
                .as_ref()
                .map(|s| s.source_lang.clone())
                .unwrap_or_else(|| {
                    self.config()
                        .get("source_lang")
                        .and_then(|v| v.as_str())
                        .unwrap_or("auto")
                        .into()
                });
            let provider = event.segment.as_ref().and_then(|s| s.provider.clone());
            self.active_partial = Some(json!({
                "sequence": event.sequence,
                "text": event.text,
                "source_lang": source_lang,
                "provider": provider,
            }));
            self.log.transcript_partial(&event);
            if let Some(completed) = self.completed_sequence {
                let (source_visible, translation_visible) =
                    self.completed_visibility(None, presentation);
                self.log
                    .ttl_visibility(completed, source_visible, translation_visible);
            }
            return;
        }

        let segment = event.segment.as_ref();
        let duration_ms = segment.and_then(|s| match (s.start_ms, s.end_ms) {
            (Some(start), Some(end)) => Some((end - start).max(0)),
            (_, Some(end)) => Some(end),
            _ => None,
        });
        let source_lang = segment.map(|s| s.source_lang.clone()).unwrap_or_else(|| {
            self.config()
                .get("source_lang")
                .and_then(|v| v.as_str())
                .unwrap_or("auto")
                .into()
        });
        let provider = segment.and_then(|s| s.provider.clone());

        self.records.insert(
            event.sequence,
            json!({
                "sequence": event.sequence,
                "source_text": event.text,
                "source_lang": source_lang,
                "translations": {},
                "provider": provider,
                "translation_received": !self.translation_required_for_display(),
                "duration_ms": duration_ms,
                "finalized_at_utc": utc_now_iso(),
                "finalized_at_monotonic": self.monotonic_now(),
            }),
        );
        self.active_partial = Some(json!({
            "sequence": event.sequence,
            "text": event.text,
            "source_lang": source_lang,
            "provider": provider,
        }));
        self.pending_final_sequence = Some(event.sequence);
        if self.latest_final_sequence.is_none()
            || event.sequence > self.latest_final_sequence.unwrap_or(0)
        {
            self.latest_final_sequence = Some(event.sequence);
        }
        self.log.transcript_final(&event);
        self.log.lifecycle_config(&self.lifecycle_config());
        self.prune_old_records();
        self.promote_or_defer(event.sequence, presentation);
    }

    pub fn handle_translation(
        &mut self,
        event: TranslationEvent,
        presentation: &SubtitlePresentation,
    ) {
        let config = self.config();
        let translation_config = config.get("translation").cloned().unwrap_or(Value::Null);
        let language_to_slot =
            SubtitlePresentation::legacy_language_to_slot_map(&translation_config);
        let slot_map = SubtitlePresentation::translation_slot_map(&translation_config);
        let required_slot_ids: Vec<String> = slot_map.keys().cloned().collect();

        let record = self.records.entry(event.sequence).or_insert_with(|| {
            json!({
                "sequence": event.sequence,
                "source_text": event.source_text,
                "source_lang": event.source_lang,
                "translations": {},
                "provider": event.provider,
                "translation_received": true,
                "duration_ms": null,
                "finalized_at_utc": null,
                "finalized_at_monotonic": null,
            })
        });

        if let Some(obj) = record.as_object_mut() {
            obj.insert("source_text".into(), json!(event.source_text));
            obj.insert("source_lang".into(), json!(event.source_lang));
            obj.insert("provider".into(), json!(event.provider));

            let mut translations = obj
                .get("translations")
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();

            for item in &event.translations {
                let mut slot_id = item
                    .slot_id
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase();
                if slot_id.is_empty() {
                    let target = item.target_lang.trim().to_ascii_lowercase();
                    slot_id = language_to_slot.get(&target).cloned().unwrap_or_default();
                }
                let translation_key = if slot_id.is_empty() {
                    item.target_lang.trim().to_ascii_lowercase()
                } else {
                    slot_id.clone()
                };
                if translation_key.is_empty() {
                    continue;
                }
                translations.insert(
                    translation_key.clone(),
                    json!({
                        "slot_id": if slot_id.is_empty() { item.slot_id.clone() } else { Some(slot_id) },
                        "target_lang": item.target_lang,
                        "label": item.label,
                        "text": item.text,
                        "provider": item.provider,
                        "success": item.success,
                        "error": item.error,
                    }),
                );
            }
            obj.insert("translations".into(), Value::Object(translations.clone()));

            let received_targets: HashSet<String> = translations.keys().cloned().collect();
            let translation_received = event.is_complete
                || required_slot_ids.is_empty()
                || required_slot_ids
                    .iter()
                    .all(|slot| received_targets.contains(slot));
            obj.insert("translation_received".into(), json!(translation_received));
        }

        let should_promote = self.pending_final_sequence == Some(event.sequence)
            || self.completed_sequence == Some(event.sequence)
            || (self.pending_final_sequence.is_none()
                && self.completed_sequence.is_none()
                && self.latest_final_sequence == Some(event.sequence));

        self.log.translation_received(&event);
        if should_promote {
            self.promote_or_defer(event.sequence, presentation);
        }
        self.maybe_flush_pending_when_unblocked(presentation);
    }

    pub fn is_sequence_relevant_for_presentation(
        &self,
        sequence: u64,
        presentation: &SubtitlePresentation,
    ) -> bool {
        if !self.records.contains_key(&sequence) {
            return false;
        }
        if self.pending_final_sequence == Some(sequence) {
            return true;
        }
        if self.completed_sequence == Some(sequence) {
            if self
                .current_completed_payload(false, presentation)
                .is_some()
            {
                return true;
            }
            return self.sequence_can_accept_late_translation(sequence, presentation);
        }
        if self.completed_sequence.is_none()
            && self.pending_final_sequence.is_none()
            && self.latest_final_sequence == Some(sequence)
            && let Some(payload) = self.promotion_payload(sequence, presentation)
        {
            return !payload.visible_items.is_empty();
        }
        false
    }

    pub fn is_sequence_relevant_for_translation(
        &self,
        sequence: u64,
        presentation: &SubtitlePresentation,
    ) -> bool {
        if !self.records.contains_key(&sequence) {
            return false;
        }
        if self.is_sequence_relevant_for_presentation(sequence, presentation) {
            return true;
        }
        if self.pending_final_sequence == Some(sequence) {
            return true;
        }
        if self.completed_sequence == Some(sequence)
            && self.sequence_awaits_translation(sequence, presentation)
        {
            return true;
        }
        if self.latest_final_sequence == Some(sequence)
            && self.sequence_can_accept_late_translation(sequence, presentation)
        {
            return true;
        }
        false
    }

    pub fn current_completed_payload(
        &self,
        hide_source: bool,
        presentation: &SubtitlePresentation,
    ) -> Option<SubtitlePayloadEvent> {
        let sequence = self.completed_sequence?;
        let payload = presentation.build_payload(sequence, self.records.get(&sequence)?);
        let (source_visible, translation_visible) = self.completed_visibility(None, presentation);
        let source_visible = source_visible && !hide_source;
        let mut remapped_items = Vec::new();
        let mut remapped_visible = Vec::new();
        for item in &payload.items {
            let mut should_show = item.visible && !item.text.is_empty();
            if item.kind == "source" {
                should_show = should_show && source_visible;
            } else {
                should_show = should_show && translation_visible;
            }
            let updated = SubtitleLineItem {
                visible: should_show,
                style_slot: if should_show {
                    item.style_slot.clone()
                } else {
                    None
                },
                ..item.clone()
            };
            remapped_items.push(updated.clone());
            if updated.visible && !updated.text.is_empty() {
                remapped_visible.push(updated);
            }
        }
        if remapped_visible.is_empty() {
            let (source_visible, translation_visible) =
                self.completed_visibility(None, presentation);
            self.log
                .ttl_visibility(sequence, source_visible, translation_visible);
            self.log.completed_payload_hidden(sequence, hide_source);
            return None;
        }
        let line1 = remapped_visible
            .first()
            .map(|i| i.text.clone())
            .unwrap_or_default();
        let line2 = if remapped_visible.len() > 1 {
            remapped_visible[1..]
                .iter()
                .map(|i| i.text.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            String::new()
        };
        Some(SubtitlePayloadEvent {
            items: remapped_items,
            visible_items: remapped_visible.clone(),
            line1,
            line2,
            ..payload
        })
    }

    pub fn on_expiry_tick(&mut self, sequence: u64, presentation: &SubtitlePresentation) {
        if self.completed_sequence != Some(sequence) {
            return;
        }
        self.expiry_task = None;
        if self
            .current_completed_payload(false, presentation)
            .is_some()
            || self.promotion_payload(sequence, presentation).is_some()
            || self.sequence_has_incomplete_translations(sequence, presentation)
        {
            self.log.ttl_expiry_tick(sequence, "reschedule", json!({}));
            self.schedule_next_expiry_check(presentation);
            self.maybe_flush_pending_when_unblocked(presentation);
            return;
        }
        self.log
            .ttl_expiry_tick(sequence, "completed_block_cleared", json!({}));
        self.completed_sequence = None;
        self.completed_expires_at_monotonic = None;
        self.completed_expires_at_utc = None;
        self.completed_source_expires_at_monotonic = None;
        self.completed_translation_expires_at_monotonic = None;
        if let Some(pending) = self.pending_final_sequence {
            self.promote_or_defer(pending, presentation);
        }
    }

    fn monotonic_now(&self) -> f64 {
        self.start_instant.elapsed().as_secs_f64()
    }

    /// Drop oldest finalized records that are no longer needed for display or expiry.
    fn prune_old_records(&mut self) {
        if self.records.len() <= LIFECYCLE_RECORDS_MAX {
            return;
        }
        let mut protected = HashSet::new();
        if let Some(seq) = self.completed_sequence {
            protected.insert(seq);
        }
        if let Some(seq) = self.pending_final_sequence {
            protected.insert(seq);
        }
        if let Some(seq) = self.latest_final_sequence {
            protected.insert(seq);
        }
        if let Some(partial) = self.active_partial.as_ref()
            && let Some(seq) = partial.get("sequence").and_then(|v| v.as_u64())
        {
            protected.insert(seq);
        }

        let mut removable: Vec<u64> = self
            .records
            .keys()
            .copied()
            .filter(|seq| !protected.contains(seq))
            .collect();
        removable.sort_unstable();

        while self.records.len() > LIFECYCLE_RECORDS_MAX && !removable.is_empty() {
            if let Some(seq) = removable.first().copied() {
                removable.remove(0);
                self.records.remove(&seq);
            } else {
                break;
            }
        }
    }

    fn translation_required_for_display(&self) -> bool {
        let config = self.config();
        let translation_config = config.get("translation");
        let subtitle_output = config.get("subtitle_output");
        let Some(translation_config) = translation_config.and_then(|v| v.as_object()) else {
            return false;
        };
        let Some(subtitle_output) = subtitle_output.and_then(|v| v.as_object()) else {
            return false;
        };
        let lines = translation_config
            .get("lines")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let enabled_lines: Vec<_> = lines
            .iter()
            .filter(|line| {
                line.as_object().is_some_and(|obj| {
                    obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true)
                        && obj
                            .get("slot_id")
                            .and_then(|v| v.as_str())
                            .is_some_and(|s| !s.trim().is_empty())
                        && obj
                            .get("target_lang")
                            .and_then(|v| v.as_str())
                            .is_some_and(|s| !s.trim().is_empty())
                })
            })
            .collect();
        translation_config
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            && subtitle_output
                .get("show_translations")
                .and_then(|v| v.as_bool())
                .unwrap_or(true)
            && subtitle_output
                .get("max_translation_languages")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                > 0
            && !enabled_lines.is_empty()
    }

    fn completed_visibility(
        &self,
        now_monotonic: Option<f64>,
        presentation: &SubtitlePresentation,
    ) -> (bool, bool) {
        let sequence = match self.completed_sequence {
            Some(s) => s,
            None => return (false, false),
        };
        let record = match self.records.get(&sequence) {
            Some(r) => r,
            None => return (false, false),
        };
        let finalized_at = record
            .get("finalized_at_monotonic")
            .and_then(|v| v.as_f64());
        let Some(finalized_at) = finalized_at else {
            return (true, true);
        };
        let lifecycle = self.lifecycle_config();
        let current = now_monotonic.unwrap_or_else(|| self.monotonic_now());
        let mut source_ttl_ms = lifecycle
            .get("completed_source_ttl_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(4500);
        let translation_ttl_ms = lifecycle
            .get("completed_translation_ttl_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(4500);
        let payload = presentation.build_payload(sequence, record);
        let has_visible_translation = payload
            .visible_items
            .iter()
            .any(|item| item.kind == "translation" && item.visible && !item.text.is_empty());
        if lifecycle
            .get("sync_source_and_translation_expiry")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
            && has_visible_translation
        {
            source_ttl_ms = source_ttl_ms.max(translation_ttl_ms);
        }
        let (source_expiry, translation_expiry) =
            if self.completed_source_expires_at_monotonic.is_some()
                || self.completed_translation_expires_at_monotonic.is_some()
            {
                (
                    self.completed_source_expires_at_monotonic
                        .unwrap_or(f64::NEG_INFINITY),
                    self.completed_translation_expires_at_monotonic
                        .unwrap_or(f64::NEG_INFINITY),
                )
            } else {
                (
                    finalized_at + source_ttl_ms as f64 / 1000.0,
                    finalized_at + translation_ttl_ms as f64 / 1000.0,
                )
            };
        (current < source_expiry, current < translation_expiry)
    }

    fn source_ttl_expired_for_sequence(&self, sequence: u64, now: Option<f64>) -> bool {
        let record = match self.records.get(&sequence) {
            Some(r) => r,
            None => return false,
        };
        let finalized_at = match record
            .get("finalized_at_monotonic")
            .and_then(|v| v.as_f64())
        {
            Some(v) => v,
            None => return false,
        };
        let ttl = self
            .lifecycle_config()
            .get("completed_source_ttl_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(4500) as f64
            / 1000.0;
        let current = now.unwrap_or_else(|| self.monotonic_now());
        current >= finalized_at + ttl
    }

    fn translation_ttl_expired_for_sequence(&self, sequence: u64) -> bool {
        let record = match self.records.get(&sequence) {
            Some(r) => r,
            None => return false,
        };
        let finalized_at = match record
            .get("finalized_at_monotonic")
            .and_then(|v| v.as_f64())
        {
            Some(v) => v,
            None => return false,
        };
        let ttl = self
            .lifecycle_config()
            .get("completed_translation_ttl_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(4500) as f64
            / 1000.0;
        self.monotonic_now() >= finalized_at + ttl
    }

    fn sequence_awaits_translation(
        &self,
        sequence: u64,
        _presentation: &SubtitlePresentation,
    ) -> bool {
        if !self.translation_required_for_display() {
            return false;
        }
        let record = match self.records.get(&sequence) {
            Some(r) => r,
            None => return false,
        };
        if record
            .get("translation_received")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return false;
        }
        !self.source_ttl_expired_for_sequence(sequence, None)
    }

    fn sequence_has_incomplete_translations(
        &self,
        sequence: u64,
        _presentation: &SubtitlePresentation,
    ) -> bool {
        if !self.translation_required_for_display() {
            return false;
        }
        let record = match self.records.get(&sequence) {
            Some(r) => r,
            None => return false,
        };
        if record
            .get("translation_received")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return false;
        }
        if self.translation_ttl_expired_for_sequence(sequence) {
            return false;
        }
        let translation_config = self
            .config()
            .get("translation")
            .cloned()
            .unwrap_or(Value::Null);
        let required_slots: Vec<String> =
            SubtitlePresentation::translation_slot_map(&translation_config)
                .keys()
                .cloned()
                .collect();
        let received: HashSet<String> = record
            .get("translations")
            .and_then(|v| v.as_object())
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default();
        if received.is_empty() {
            return false;
        }
        if required_slots.is_empty() {
            return true;
        }
        !required_slots.iter().all(|slot| received.contains(slot))
    }

    fn sequence_can_accept_late_translation(
        &self,
        sequence: u64,
        _presentation: &SubtitlePresentation,
    ) -> bool {
        if !self.translation_required_for_display() {
            return false;
        }
        let record = match self.records.get(&sequence) {
            Some(r) => r,
            None => return false,
        };
        if record
            .get("translation_received")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return false;
        }
        !self.translation_ttl_expired_for_sequence(sequence)
    }

    fn promotion_payload(
        &self,
        sequence: u64,
        presentation: &SubtitlePresentation,
    ) -> Option<SubtitlePayloadEvent> {
        let record = self.records.get(&sequence)?;
        let mut payload = presentation.build_payload(sequence, record);
        if self.source_ttl_expired_for_sequence(sequence, None) {
            let mut remapped_items = Vec::new();
            let mut remapped_visible = Vec::new();
            for item in &payload.items {
                let should_show = item.visible && !item.text.is_empty() && item.kind != "source";
                let updated = SubtitleLineItem {
                    visible: should_show,
                    style_slot: if should_show {
                        item.style_slot.clone()
                    } else {
                        None
                    },
                    ..item.clone()
                };
                remapped_items.push(updated.clone());
                if updated.visible && !updated.text.is_empty() {
                    remapped_visible.push(updated);
                }
            }
            if remapped_visible.is_empty() {
                return None;
            }
            let line1 = remapped_visible
                .first()
                .map(|i| i.text.clone())
                .unwrap_or_default();
            let line2 = if remapped_visible.len() > 1 {
                remapped_visible[1..]
                    .iter()
                    .map(|i| i.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                String::new()
            };
            payload.items = remapped_items;
            payload.visible_items = remapped_visible.clone();
            payload.line1 = line1;
            payload.line2 = line2;
        }
        if payload.visible_items.is_empty() {
            None
        } else {
            Some(payload)
        }
    }

    fn maybe_flush_pending_when_unblocked(&mut self, presentation: &SubtitlePresentation) {
        let pending = match self.pending_final_sequence {
            Some(pending) => pending,
            None => return,
        };
        match self.completed_sequence {
            None => self.promote_or_defer(pending, presentation),
            Some(completed) if completed == pending => {}
            Some(completed) => {
                if !self.sequence_awaits_translation(completed, presentation) {
                    self.promote_or_defer(pending, presentation);
                }
            }
        }
    }

    fn promote_or_defer(&mut self, sequence: u64, presentation: &SubtitlePresentation) {
        let payload = if let Some(p) = self.promotion_payload(sequence, presentation) { p } else {
            self.pending_final_sequence = Some(sequence);
            self.log.promote_or_defer(
                sequence,
                "deferred",
                "promotion_payload_empty",
                json!({ "pending_final_sequence": sequence }),
            );
            return;
        };
        let lifecycle = self.lifecycle_config();
        if let Some(completed) = self.completed_sequence {
            if completed != sequence && self.sequence_awaits_translation(completed, presentation) {
                self.pending_final_sequence = Some(sequence);
                self.log.promote_or_defer(
                    sequence,
                    "deferred",
                    "previous_sequence_awaits_translation",
                    json!({ "completed_sequence": completed }),
                );
                return;
            }
            if completed != sequence
                && !lifecycle
                    .get("allow_early_replace_on_next_final")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true)
            {
                self.pending_final_sequence = Some(sequence);
                self.log.promote_or_defer(
                    sequence,
                    "deferred",
                    "allow_early_replace_on_next_final_false",
                    json!({ "completed_sequence": completed }),
                );
                return;
            }
        }
        let preserved_pending = self
            .pending_final_sequence
            .filter(|&pending| pending != sequence);
        self.completed_sequence = Some(sequence);
        self.pending_final_sequence = preserved_pending;
        if self
            .active_partial
            .as_ref()
            .and_then(|p| p.get("sequence"))
            .and_then(|v| v.as_u64())
            == Some(sequence)
        {
            self.active_partial = None;
        }
        self.log.promote_or_defer(
            sequence,
            "promoted",
            "completed_sequence_updated",
            json!({
                "completed_sequence": sequence,
                "pending_final_sequence": self.pending_final_sequence,
                "visible_texts": payload.visible_items.iter().map(|item| item.text.as_str()).collect::<Vec<_>>(),
            }),
        );
        self.schedule_expiry_internal(&payload, presentation);
    }

    fn schedule_expiry_internal(
        &mut self,
        payload: &SubtitlePayloadEvent,
        presentation: &SubtitlePresentation,
    ) {
        if let Some(task) = self.expiry_task.take() {
            task.abort();
        }
        if self.completed_sequence.is_none() {
            self.completed_expires_at_utc = None;
            self.completed_expires_at_monotonic = None;
            self.completed_source_expires_at_monotonic = None;
            self.completed_translation_expires_at_monotonic = None;
            return;
        }
        let lifecycle = self.lifecycle_config();
        let has_visible_source = payload
            .visible_items
            .iter()
            .any(|i| i.kind == "source" && i.visible && !i.text.is_empty());
        let has_visible_translation = payload
            .visible_items
            .iter()
            .any(|i| i.kind == "translation" && i.visible && !i.text.is_empty());
        let now = self.monotonic_now();
        let mut source_ttl_ms = lifecycle
            .get("completed_source_ttl_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(4500);
        let translation_ttl_ms = lifecycle
            .get("completed_translation_ttl_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(4500);
        if lifecycle
            .get("sync_source_and_translation_expiry")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
            && has_visible_translation
        {
            source_ttl_ms = source_ttl_ms.max(translation_ttl_ms);
        }
        self.completed_source_expires_at_monotonic = Some(if has_visible_source {
            now + source_ttl_ms as f64 / 1000.0
        } else {
            now - 0.001
        });
        self.completed_translation_expires_at_monotonic = Some(if has_visible_translation {
            now + translation_ttl_ms as f64 / 1000.0
        } else {
            now - 0.001
        });
        self.log.ttl_schedule(json!({
            "completed_sequence": self.completed_sequence,
            "source_ttl_ms": source_ttl_ms,
            "translation_ttl_ms": translation_ttl_ms,
            "sync_source_and_translation_expiry": lifecycle
                .get("sync_source_and_translation_expiry")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            "has_visible_source": has_visible_source,
            "has_visible_translation": has_visible_translation,
            "source_expires_at_monotonic": self.completed_source_expires_at_monotonic,
            "translation_expires_at_monotonic": self.completed_translation_expires_at_monotonic,
            "completed_expires_at_utc": self.completed_expires_at_utc,
        }));
        self.schedule_next_expiry_check(presentation);
    }

    fn schedule_next_expiry_check(&mut self, presentation: &SubtitlePresentation) {
        let now = self.monotonic_now();
        let expiry_points: Vec<f64> = [
            self.completed_source_expires_at_monotonic,
            self.completed_translation_expires_at_monotonic,
        ]
        .into_iter()
        .flatten()
        .filter(|point| *point > now)
        .collect();
        if expiry_points.is_empty() {
            self.completed_expires_at_utc = None;
            self.completed_expires_at_monotonic = None;
            self.expiry_task = None;
            return;
        }
        self.completed_expires_at_monotonic = expiry_points.iter().copied().reduce(f64::max);
        if let Some(max_point) = self.completed_expires_at_monotonic {
            let delta_ms = ((max_point - now) * 1000.0).round() as i64;
            self.completed_expires_at_utc = Some(format_expires_utc(delta_ms));
            let next_check = expiry_points
                .iter()
                .copied()
                .reduce(f64::min)
                .unwrap_or(max_point);
            let sequence = self.completed_sequence.unwrap_or(0);
            let delay_secs = (next_check - now).max(0.0);
            (self.schedule_expiry)(sequence, delay_secs);
        }
        let _ = presentation;
    }
}

fn utc_now_iso() -> String {
    voicesub_types::utc_now_rfc3339()
}

fn format_expires_utc(delta_ms: i64) -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .saturating_add((delta_ms / 1000).max(0) as u64);
    voicesub_types::epoch_secs_to_rfc3339(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_old_records_keeps_active_sequences() {
        let schedule = Arc::new(|_sequence: u64, _delay: f64| {});
        let mut core =
            SubtitleLifecycleCore::new(Box::new(|| json!({})), schedule, SubtitleLog::default());
        core.completed_sequence = Some(600);
        core.pending_final_sequence = Some(601);
        core.latest_final_sequence = Some(601);
        core.active_partial = Some(json!({ "sequence": 602 }));

        for seq in 1..=600u64 {
            core.records.insert(seq, json!({ "sequence": seq }));
        }
        assert_eq!(core.records.len(), 600);

        core.prune_old_records();

        assert!(core.records.contains_key(&600));
        assert!(core.records.len() <= LIFECYCLE_RECORDS_MAX);
        assert!(!core.records.contains_key(&1));
    }
}
