use std::collections::HashSet;

use serde_json::{json, Value};
use tracing::trace;

use crate::queue::SpeechQueueItem;
use crate::trace as tts_trace;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TtsSpeechSettings {
    #[serde(default = "default_speak_source")]
    pub speak_source: bool,
    #[serde(default = "default_speak_translations")]
    pub speak_translations: bool,
    /// Empty = all translation slots (when `speak_translations` is true).
    #[serde(default)]
    pub translation_slots: Vec<String>,
    #[serde(default = "default_min_chars")]
    pub min_chars: u32,
    #[serde(default = "default_max_queue_items")]
    pub max_queue_items: u32,
}

fn default_speak_source() -> bool {
    true
}

fn default_speak_translations() -> bool {
    true
}

fn default_min_chars() -> u32 {
    2
}

fn default_max_queue_items() -> u32 {
    8
}

impl Default for TtsSpeechSettings {
    fn default() -> Self {
        Self {
            speak_source: default_speak_source(),
            speak_translations: default_speak_translations(),
            translation_slots: Vec::new(),
            min_chars: default_min_chars(),
            max_queue_items: default_max_queue_items(),
        }
    }
}

fn translation_slot_allowed(settings: &TtsSpeechSettings, slot_id: &str) -> bool {
    if !settings.speak_translations {
        return false;
    }
    if settings.translation_slots.is_empty() {
        return true;
    }
    let slot = slot_id.trim().to_ascii_lowercase();
    settings
        .translation_slots
        .iter()
        .any(|entry| entry.trim().eq_ignore_ascii_case(&slot))
}

/// Keep dedupe keys for recent subtitle sequences only (long-session memory bound).
const SPOKEN_KEYS_MAX: usize = 2048;
const SPOKEN_KEYS_SEQUENCE_WINDOW: u64 = 384;

#[derive(Debug, Default)]
pub struct SubtitleSpeechPlanner {
    spoken_keys: HashSet<String>,
    max_sequence_seen: u64,
}

impl SubtitleSpeechPlanner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        let keys = self.spoken_keys.len();
        self.spoken_keys.clear();
        self.max_sequence_seen = 0;
        trace!(target: "voicesub.tts", keys, "planner spoken keys cleared");
    }

    /// Allow re-planning lines dropped from the speech queue before playback.
    pub fn release_dedupe_keys(&mut self, keys: impl IntoIterator<Item = String>) {
        let mut released = 0usize;
        for key in keys {
            if self.spoken_keys.remove(&key) {
                released += 1;
            }
        }
        if released > 0 {
            trace!(
                target: "voicesub.tts",
                released,
                remaining = self.spoken_keys.len(),
                "planner dedupe keys released after queue drop"
            );
        }
    }

    pub fn plan(&mut self, payload: &Value, settings: &TtsSpeechSettings) -> Vec<SpeechQueueItem> {
        let lifecycle = payload
            .get("lifecycle_state")
            .and_then(|v| v.as_str())
            .unwrap_or("idle");
        if lifecycle != "completed_only" {
            trace!(
                target: "voicesub.tts",
                lifecycle,
                "planner skip: lifecycle not completed_only"
            );
            return Vec::new();
        }

        let sequence = payload
            .get("sequence")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if sequence == 0 {
            trace!(target: "voicesub.tts", "planner skip: sequence is zero");
            return Vec::new();
        }

        let mut out = Vec::new();
        let items = payload
            .get("visible_items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        trace!(
            target: "voicesub.tts",
            sequence,
            visible_items = items.len(),
            "planner evaluating payload"
        );

        for item in items {
            let Some(obj) = item.as_object() else {
                continue;
            };
            let kind = obj
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("source");
            if obj.get("visible").and_then(|v| v.as_bool()) == Some(false) {
                tts_trace::trace("planner", "skip_invisible", json!({ "sequence": sequence, "kind": kind }));
                continue;
            }
            if obj.get("success").and_then(|v| v.as_bool()) == Some(false) {
                tts_trace::trace("planner", "skip_failed", json!({ "sequence": sequence, "kind": kind }));
                continue;
            }

            let text = obj
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if text.chars().count() < settings.min_chars as usize {
                tts_trace::trace(
                    "planner",
                    "skip_min_chars",
                    json!({ "sequence": sequence, "kind": kind, "min_chars": settings.min_chars }),
                );
                continue;
            }

            let slot_id = obj
                .get("slot_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let is_source = kind.eq_ignore_ascii_case("source");
            if is_source && !settings.speak_source {
                tts_trace::trace("planner", "skip_source_disabled", json!({ "sequence": sequence }));
                continue;
            }
            if !is_source && !translation_slot_allowed(settings, slot_id) {
                tts_trace::trace(
                    "planner",
                    "skip_translation_slot",
                    json!({ "sequence": sequence, "slot_id": slot_id }),
                );
                continue;
            }
            let key = speech_key(sequence, kind, slot_id, &text);
            if !self.spoken_keys.insert(key.clone()) {
                tts_trace::trace(
                    "planner",
                    "skip_dedupe",
                    json!({ "sequence": sequence, "kind": kind, "slot_id": slot_id }),
                );
                continue;
            }

            let lang = speech_lang_for_item(obj, is_source, payload);

            out.push(SpeechQueueItem {
                id: format!("sub-{sequence}-{kind}-{slot_id}"),
                text: text.clone(),
                source: if is_source {
                    "subtitle_source".to_string()
                } else {
                    format!("subtitle_{slot_id}")
                },
                lang,
                dedupe_key: Some(key),
            });

            if out.len() >= settings.max_queue_items as usize {
                break;
            }
        }

        self.maybe_prune_spoken_keys(sequence);
        out
    }

    fn maybe_prune_spoken_keys(&mut self, current_sequence: u64) {
        if current_sequence > self.max_sequence_seen {
            self.max_sequence_seen = current_sequence;
        }
        let cutoff = self.max_sequence_seen.saturating_sub(SPOKEN_KEYS_SEQUENCE_WINDOW);
        let over_capacity = self.spoken_keys.len() > SPOKEN_KEYS_MAX;
        if !over_capacity && cutoff == 0 {
            return;
        }
        let before = self.spoken_keys.len();
        self.spoken_keys
            .retain(|key| sequence_from_speech_key(key).is_some_and(|seq| seq >= cutoff));
        let removed = before.saturating_sub(self.spoken_keys.len());
        if removed > 0 {
            trace!(
                target: "voicesub.tts",
                removed,
                remaining = self.spoken_keys.len(),
                cutoff,
                max_sequence = self.max_sequence_seen,
                "planner spoken keys pruned"
            );
        }
    }
}

fn speech_key(sequence: u64, kind: &str, slot_id: &str, text: &str) -> String {
    format!(
        "{sequence}:{kind}:{slot_id}:{:016x}",
        stable_text_hash(text)
    )
}

fn sequence_from_speech_key(key: &str) -> Option<u64> {
    key.split(':').next()?.parse().ok()
}

fn stable_text_hash(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn speech_lang_for_item(item: &serde_json::Map<String, Value>, is_source: bool, payload: &Value) -> String {
    let from_item = if is_source {
        item.get("lang").and_then(|v| v.as_str())
    } else {
        item.get("target_lang")
            .or_else(|| item.get("lang"))
            .and_then(|v| v.as_str())
    };
    from_item
        .or_else(|| payload.get("source_lang").and_then(|v| v.as_str()))
        .unwrap_or("en")
        .to_string()
}

#[cfg(test)]
impl SubtitleSpeechPlanner {
    fn spoken_keys_len(&self) -> usize {
        self.spoken_keys.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn completed_payload(sequence: u64, items: Vec<Value>) -> Value {
        json!({
            "sequence": sequence,
            "event_sequence": sequence,
            "lifecycle_state": "completed_only",
            "completed_block_visible": true,
            "visible_items": items,
            "active_partial_text": ""
        })
    }

    #[test]
    fn speaks_source_and_translation_on_completed_only() {
        let mut planner = SubtitleSpeechPlanner::new();
        let settings = TtsSpeechSettings::default();
        let payload = completed_payload(
            1,
            vec![
                json!({"kind": "source", "text": "Привет", "slot_id": "", "style_slot": "source"}),
                json!({"kind": "translation", "text": "Hello", "slot_id": "translation_1", "style_slot": "translation_1"}),
            ],
        );
        let lines = planner.plan(&payload, &settings);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "Привет");
        assert_eq!(lines[1].text, "Hello");
    }

    #[test]
    fn ignores_partial_lifecycle() {
        let mut planner = SubtitleSpeechPlanner::new();
        let settings = TtsSpeechSettings::default();
        let payload = json!({
            "sequence": 2,
            "lifecycle_state": "completed_with_partial",
            "visible_items": [{"kind": "source", "text": "partial phrase"}],
            "active_partial_text": "partial phrase"
        });
        assert!(planner.plan(&payload, &settings).is_empty());
    }

    #[test]
    fn dedupes_same_sequence_and_text() {
        let mut planner = SubtitleSpeechPlanner::new();
        let settings = TtsSpeechSettings::default();
        let payload = completed_payload(1, vec![json!({"kind": "source", "text": "Hello"})]);
        assert_eq!(planner.plan(&payload, &settings).len(), 1);
        assert!(planner.plan(&payload, &settings).is_empty());
    }

    #[test]
    fn late_translation_on_same_sequence_is_spoken_once() {
        let mut planner = SubtitleSpeechPlanner::new();
        let settings = TtsSpeechSettings::default();
        let source_only = completed_payload(1, vec![json!({"kind": "source", "text": "Hi"})]);
        assert_eq!(planner.plan(&source_only, &settings).len(), 1);

        let with_translation = completed_payload(
            1,
            vec![
                json!({"kind": "source", "text": "Hi"}),
                json!({"kind": "translation", "text": "Привет", "slot_id": "translation_1"}),
            ],
        );
        let late = planner.plan(&with_translation, &settings);
        assert_eq!(late.len(), 1);
        assert_eq!(late[0].text, "Привет");
    }

    #[test]
    fn skips_invisible_and_failed_items() {
        let mut planner = SubtitleSpeechPlanner::new();
        let settings = TtsSpeechSettings::default();
        let payload = completed_payload(
            4,
            vec![
                json!({"kind": "source", "text": "Visible", "visible": true}),
                json!({"kind": "source", "text": "Hidden", "visible": false}),
                json!({
                    "kind": "translation",
                    "text": "Failed",
                    "slot_id": "translation_1",
                    "visible": true,
                    "success": false
                }),
            ],
        );
        let lines = planner.plan(&payload, &settings);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Visible");
    }

    #[test]
    fn carries_lang_from_items_and_payload() {
        let mut planner = SubtitleSpeechPlanner::new();
        let settings = TtsSpeechSettings::default();
        let payload = json!({
            "sequence": 5,
            "lifecycle_state": "completed_only",
            "completed_block_visible": true,
            "visible_items": [
                {"kind": "source", "text": "Привет", "lang": "ru"},
                {"kind": "translation", "text": "Hello", "slot_id": "translation_1", "target_lang": "en"},
            ],
            "source_lang": "ja",
            "active_partial_text": ""
        });
        let lines = planner.plan(&payload, &settings);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].lang, "ru");
        assert_eq!(lines[1].lang, "en");
    }

    #[test]
    fn filters_translation_by_selected_slots() {
        let mut planner = SubtitleSpeechPlanner::new();
        let settings = TtsSpeechSettings {
            speak_source: false,
            speak_translations: true,
            translation_slots: vec!["translation_2".into()],
            min_chars: 2,
            max_queue_items: 8,
        };
        let payload = completed_payload(
            6,
            vec![
                json!({"kind": "translation", "text": "One", "slot_id": "translation_1", "target_lang": "en"}),
                json!({"kind": "translation", "text": "Two", "slot_id": "translation_2", "target_lang": "de"}),
            ],
        );
        let lines = planner.plan(&payload, &settings);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Two");
        assert_eq!(lines[0].lang, "de");
    }

    #[test]
    fn spoken_keys_prune_keeps_recent_sequences_and_dedupe() {
        let mut planner = SubtitleSpeechPlanner::new();
        let settings = TtsSpeechSettings::default();

        for sequence in 1..=500u64 {
            let payload = completed_payload(
                sequence,
                vec![json!({"kind": "source", "text": format!("line {sequence}")})],
            );
            assert_eq!(planner.plan(&payload, &settings).len(), 1);
        }
        assert!(
            planner.spoken_keys_len() <= super::SPOKEN_KEYS_MAX,
            "spoken_keys should stay bounded, got {}",
            planner.spoken_keys_len()
        );

        let repeat = completed_payload(
            500,
            vec![json!({"kind": "source", "text": "line 500"})],
        );
        assert!(planner.plan(&repeat, &settings).is_empty());
    }

    #[test]
    fn release_dedupe_key_allows_replan() {
        let mut planner = SubtitleSpeechPlanner::new();
        let settings = TtsSpeechSettings::default();
        let payload = completed_payload(7, vec![json!({"kind": "source", "text": "Again"})]);
        let first = planner.plan(&payload, &settings);
        assert_eq!(first.len(), 1);
        let key = first[0].dedupe_key.clone().expect("dedupe key");
        assert!(planner.plan(&payload, &settings).is_empty());
        planner.release_dedupe_keys([key]);
        let second = planner.plan(&payload, &settings);
        assert_eq!(second.len(), 1);
    }

    #[test]
    fn speech_key_uses_stable_hash_not_full_text() {
        let key = speech_key(9, "source", "", "Hello");
        assert!(!key.contains("Hello"));
        assert_eq!(key, speech_key(9, "source", "", "Hello"));
        assert_ne!(key, speech_key(9, "source", "", "World"));
    }

    #[test]
    fn respects_min_chars_and_source_toggle() {
        let mut planner = SubtitleSpeechPlanner::new();
        let settings = TtsSpeechSettings {
            speak_source: false,
            speak_translations: true,
            translation_slots: Vec::new(),
            min_chars: 3,
            max_queue_items: 8,
        };
        let payload = completed_payload(
            3,
            vec![
                json!({"kind": "source", "text": "ok"}),
                json!({"kind": "translation", "text": "hi", "slot_id": "translation_1"}),
            ],
        );
        assert!(planner.plan(&payload, &settings).is_empty());
    }
}
