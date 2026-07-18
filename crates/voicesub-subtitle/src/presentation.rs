use std::collections::HashMap;

use serde_json::Value;

use crate::lifecycle::SubtitleLifecycleCore;
use crate::style::resolve_effective_subtitle_style;
use crate::trace::SubtitleLog;
use crate::types::{LifecycleState, SubtitleLineItem, SubtitlePayloadEvent};

pub(crate) struct SubtitlePresentation {
    config_getter: Box<dyn Fn() -> Value + Send + Sync>,
    stale_translation_suppressed: Box<dyn Fn(u64) + Send + Sync>,
    payload_mismatch_count: Box<dyn Fn(u64) + Send + Sync>,
    log: SubtitleLog,
}

impl SubtitlePresentation {
    pub fn new(
        config_getter: Box<dyn Fn() -> Value + Send + Sync>,
        stale_translation_suppressed: Box<dyn Fn(u64) + Send + Sync>,
        payload_mismatch_count: Box<dyn Fn(u64) + Send + Sync>,
        log: SubtitleLog,
    ) -> Self {
        Self {
            config_getter,
            stale_translation_suppressed,
            payload_mismatch_count,
            log,
        }
    }

    pub fn config(&self) -> Value {
        (self.config_getter)()
    }

    pub fn translation_slot_map(translation_config: &Value) -> HashMap<String, Value> {
        let mut out = HashMap::new();
        let Some(lines) = translation_config.get("lines").and_then(|v| v.as_array()) else {
            return out;
        };
        for line in lines {
            let Some(obj) = line.as_object() else {
                continue;
            };
            if !obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true) {
                continue;
            }
            let slot_id = obj
                .get("slot_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            let target_lang = obj
                .get("target_lang")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if slot_id.is_empty() || target_lang.is_empty() {
                continue;
            }
            out.insert(slot_id, line.clone());
        }
        out
    }

    /// Enabled slot ids in `translation.lines` array order (stable; not HashMap iteration).
    pub fn enabled_slot_ids_in_order(translation_config: &Value) -> Vec<String> {
        let mut out = Vec::new();
        let Some(lines) = translation_config.get("lines").and_then(|v| v.as_array()) else {
            return out;
        };
        for line in lines {
            let Some(obj) = line.as_object() else {
                continue;
            };
            if !obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true) {
                continue;
            }
            let slot_id = obj
                .get("slot_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            let target_lang = obj
                .get("target_lang")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if slot_id.is_empty() || target_lang.is_empty() {
                continue;
            }
            if !out.iter().any(|existing| existing == &slot_id) {
                out.push(slot_id);
            }
        }
        out
    }

    pub fn legacy_language_to_slot_map(translation_config: &Value) -> HashMap<String, String> {
        let mut out = HashMap::new();
        let Some(lines) = translation_config.get("lines").and_then(|v| v.as_array()) else {
            return out;
        };
        for line in lines {
            let Some(obj) = line.as_object() else {
                continue;
            };
            if !obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true) {
                continue;
            }
            let target_lang = obj
                .get("target_lang")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            let slot_id = obj
                .get("slot_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            if !target_lang.is_empty() && !slot_id.is_empty() && !out.contains_key(&target_lang) {
                out.insert(target_lang, slot_id);
            }
        }
        out
    }

    pub fn resolved_display_order(
        translation_config: &Value,
        subtitle_output: &Value,
    ) -> Vec<String> {
        let enabled_slots = Self::enabled_slot_ids_in_order(translation_config);
        let language_to_slot = Self::legacy_language_to_slot_map(translation_config);
        let raw = subtitle_output
            .get("display_order")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_else(|| {
                let mut order = vec![Value::String("source".into())];
                for slot in &enabled_slots {
                    order.push(Value::String(slot.clone()));
                }
                order
            });

        let mut normalized = Vec::new();
        for item in raw {
            let value = item.as_str().unwrap_or("").trim().to_ascii_lowercase();
            if value == "source" {
                if !normalized.contains(&"source".to_string()) {
                    normalized.push("source".into());
                }
                continue;
            }
            if enabled_slots.contains(&value) && !normalized.contains(&value) {
                normalized.push(value);
                continue;
            }
            if let Some(mapped) = language_to_slot.get(&value)
                && !normalized.contains(mapped)
            {
                normalized.push(mapped.clone());
            }
        }
        if !normalized.contains(&"source".to_string()) {
            normalized.push("source".into());
        }
        for slot in enabled_slots {
            if !normalized.contains(&slot) {
                normalized.push(slot);
            }
        }
        normalized
    }

    pub fn build_payload(&self, sequence: u64, record: &Value) -> SubtitlePayloadEvent {
        let config = self.config();
        let translation_config = config.get("translation").cloned().unwrap_or(Value::Null);
        let subtitle_output = config
            .get("subtitle_output")
            .cloned()
            .unwrap_or(Value::Null);
        let overlay = config.get("overlay").cloned().unwrap_or(Value::Null);
        let subtitle_style = config.get("subtitle_style").cloned().unwrap_or(Value::Null);

        let show_source = subtitle_output
            .get("show_source")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let show_translations = subtitle_output
            .get("show_translations")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let max_translation_languages = subtitle_output
            .get("max_translation_languages")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            .min(5) as u32;
        let translation_slots = Self::translation_slot_map(&translation_config);
        let display_order = Self::resolved_display_order(&translation_config, &subtitle_output);

        let source_lang = record
            .get("source_lang")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");
        let source_text = record
            .get("source_text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let translations = record.get("translations").and_then(|v| v.as_object());

        let mut items = Vec::new();
        let mut visible_items = Vec::new();

        for code in &display_order {
            if code == "source" {
                let source_item = SubtitleLineItem {
                    kind: "source".into(),
                    lang: source_lang.into(),
                    label: source_lang.to_ascii_uppercase(),
                    text: source_text.into(),
                    style_slot: Some("source".into()),
                    slot_id: None,
                    target_lang: None,
                    provider: None,
                    visible: show_source,
                    success: true,
                    error: None,
                };
                if source_item.visible && !source_item.text.is_empty() {
                    visible_items.push(source_item.clone());
                }
                items.push(source_item);
                continue;
            }

            let Some(line_config) = translation_slots.get(code) else {
                continue;
            };
            let translation = translations.and_then(|m| m.get(code));
            let success = translation
                .and_then(|t| t.get("success"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let text = translation
                .and_then(|t| t.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let error = translation
                .and_then(|t| t.get("error"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let translation_count = visible_items
                .iter()
                .filter(|i| i.kind == "translation")
                .count();
            let can_show = show_translations
                && translation_count < max_translation_languages as usize
                && success
                && !text.is_empty();

            let target_lang = translation
                .and_then(|t| t.get("target_lang"))
                .and_then(|v| v.as_str())
                .or_else(|| line_config.get("target_lang").and_then(|v| v.as_str()))
                .unwrap_or(code);
            let label = translation
                .and_then(|t| t.get("label"))
                .and_then(|v| v.as_str())
                .or_else(|| line_config.get("label").and_then(|v| v.as_str()))
                .unwrap_or(&target_lang.to_ascii_uppercase())
                .to_string();

            let item = SubtitleLineItem {
                kind: "translation".into(),
                lang: target_lang.into(),
                label,
                text: text.into(),
                style_slot: if can_show { Some(code.clone()) } else { None },
                slot_id: Some(code.clone()),
                target_lang: Some(target_lang.into()),
                provider: translation
                    .and_then(|t| t.get("provider"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                visible: can_show,
                success,
                error,
            };
            items.push(item.clone());
            if can_show {
                visible_items.push(item);
            }
        }

        let line1 = visible_items
            .first()
            .map(|i| i.text.clone())
            .unwrap_or_default();
        let line2 = if visible_items.len() > 1 {
            visible_items[1..]
                .iter()
                .map(|i| i.text.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            String::new()
        };

        SubtitlePayloadEvent {
            sequence,
            completed_sequence: Some(sequence),
            source_lang: source_lang.into(),
            source_text: source_text.into(),
            provider: record
                .get("provider")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            preset: overlay
                .get("preset")
                .and_then(|v| v.as_str())
                .unwrap_or("single")
                .into(),
            compact: overlay
                .get("compact")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            display_order,
            show_source,
            show_translations,
            max_translation_languages,
            items,
            visible_items,
            style: resolve_effective_subtitle_style(&subtitle_style),
            lifecycle_state: LifecycleState::Idle,
            completed_block_visible: false,
            completed_expires_at_utc: None,
            active_partial_text: String::new(),
            active_partial_sequence: None,
            active_partial_source_lang: None,
            line1,
            line2,
            created_at_ms: None,
        }
    }

    pub fn should_suppress_source_partial_display(&self) -> bool {
        let config = self.config();
        !config
            .get("subtitle_output")
            .and_then(|v| v.get("show_source"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }

    pub fn build_presentation_payload(&self, core: &SubtitleLifecycleCore) -> SubtitlePayloadEvent {
        let completed_payload = core.current_completed_payload(false, self);
        let active_partial = core.active_partial();
        let active_partial_text = active_partial
            .as_ref()
            .and_then(|p| p.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let active_partial_sequence = active_partial
            .as_ref()
            .and_then(|p| p.get("sequence"))
            .and_then(|v| v.as_u64());
        let active_partial_source_lang = active_partial
            .as_ref()
            .and_then(|p| p.get("source_lang"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let display_partial_source = !self.should_suppress_source_partial_display();
        let visible_partial_text = if display_partial_source {
            active_partial_text.clone()
        } else {
            String::new()
        };

        let completed_translation_payload = if active_partial_text.is_empty() {
            completed_payload.clone()
        } else {
            core.current_completed_payload(true, self)
        };

        let lifecycle_state =
            if !active_partial_text.is_empty() && completed_translation_payload.is_some() {
                LifecycleState::CompletedWithPartial
            } else if !active_partial_text.is_empty() {
                LifecycleState::PartialOnly
            } else if completed_payload.is_some() {
                LifecycleState::CompletedOnly
            } else {
                LifecycleState::Idle
            };

        let config = self.config();
        let overlay = config.get("overlay").cloned().unwrap_or(Value::Null);
        let translation_config = config.get("translation").cloned().unwrap_or(Value::Null);
        let subtitle_output = config
            .get("subtitle_output")
            .cloned()
            .unwrap_or(Value::Null);
        let display_order = Self::resolved_display_order(&translation_config, &subtitle_output);

        if !active_partial_text.is_empty() && completed_translation_payload.is_none() {
            let show_source = subtitle_output
                .get("show_source")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let source_lang = active_partial_source_lang.clone().unwrap_or_else(|| {
                config
                    .get("source_lang")
                    .and_then(|v| v.as_str())
                    .unwrap_or("auto")
                    .into()
            });
            let source_item = SubtitleLineItem {
                kind: "source".into(),
                lang: source_lang.clone(),
                label: source_lang.to_ascii_uppercase(),
                text: active_partial_text.clone(),
                style_slot: if show_source && !active_partial_text.is_empty() {
                    Some("source".into())
                } else {
                    None
                },
                slot_id: None,
                target_lang: None,
                provider: active_partial
                    .as_ref()
                    .and_then(|p| p.get("provider"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                visible: show_source && !active_partial_text.is_empty(),
                success: true,
                error: None,
            };
            let visible_items: Vec<_> = if source_item.visible && !source_item.text.is_empty() {
                vec![source_item.clone()]
            } else {
                vec![]
            };
            let line1 = visible_items
                .first()
                .map(|i| i.text.clone())
                .unwrap_or_default();
            let payload = SubtitlePayloadEvent {
                sequence: active_partial_sequence.unwrap_or(0),
                completed_sequence: core.completed_sequence(),
                source_lang,
                source_text: active_partial_text,
                provider: source_item.provider.clone(),
                preset: overlay
                    .get("preset")
                    .and_then(|v| v.as_str())
                    .unwrap_or("single")
                    .into(),
                compact: overlay
                    .get("compact")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                display_order,
                show_source,
                show_translations: subtitle_output
                    .get("show_translations")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
                max_translation_languages: subtitle_output
                    .get("max_translation_languages")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
                    .min(5) as u32,
                style: resolve_effective_subtitle_style(
                    &config.get("subtitle_style").cloned().unwrap_or(Value::Null),
                ),
                lifecycle_state,
                completed_block_visible: false,
                completed_expires_at_utc: core.completed_expires_at_utc(),
                active_partial_text: visible_partial_text,
                active_partial_sequence,
                active_partial_source_lang,
                items: vec![source_item],
                visible_items,
                line1,
                line2: String::new(),
                created_at_ms: None,
            };
            self.log
                .presentation_built(lifecycle_state, true, false, &payload);
            return payload;
        }

        let completed_tp_snapshot = completed_translation_payload.clone();
        let mut payload = completed_translation_payload.unwrap_or_else(|| SubtitlePayloadEvent {
            preset: overlay
                .get("preset")
                .and_then(|v| v.as_str())
                .unwrap_or("single")
                .into(),
            compact: overlay
                .get("compact")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            display_order: display_order.clone(),
            show_source: subtitle_output
                .get("show_source")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            show_translations: subtitle_output
                .get("show_translations")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            max_translation_languages: subtitle_output
                .get("max_translation_languages")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                .min(5) as u32,
            style: resolve_effective_subtitle_style(
                &config.get("subtitle_style").cloned().unwrap_or(Value::Null),
            ),
            ..Default::default()
        });

        if !active_partial_text.is_empty()
            && let Some(completed_tp) = completed_tp_snapshot.as_ref()
        {
            let preserve = core
                .lifecycle_config()
                .get("keep_completed_translation_during_active_partial")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            if !preserve && !completed_tp.visible_items.is_empty() {
                (self.payload_mismatch_count)(1);
            }

            let show_source = subtitle_output
                .get("show_source")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let show_translations = subtitle_output
                .get("show_translations")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let max_translation_languages = subtitle_output
                .get("max_translation_languages")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                .min(5) as u32;
            let active_source_lang = active_partial_source_lang
                .clone()
                .unwrap_or_else(|| payload.source_lang.clone());

            let source_item = SubtitleLineItem {
                kind: "source".into(),
                lang: active_source_lang.clone(),
                label: active_source_lang.to_ascii_uppercase(),
                text: active_partial_text.clone(),
                style_slot: if show_source && !active_partial_text.is_empty() {
                    Some("source".into())
                } else {
                    None
                },
                slot_id: None,
                target_lang: None,
                provider: active_partial
                    .as_ref()
                    .and_then(|p| p.get("provider"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                visible: show_source && !active_partial_text.is_empty(),
                success: true,
                error: None,
            };

            let mut items = Vec::new();
            let mut visible_items = Vec::new();
            for code in &display_order {
                if code == "source" {
                    items.push(source_item.clone());
                    if source_item.visible && !source_item.text.is_empty() {
                        visible_items.push(source_item.clone());
                    }
                    continue;
                }
                let translation_item = payload.items.iter().find(|item| {
                    item.kind == "translation"
                        && item
                            .slot_id
                            .as_deref()
                            .unwrap_or(&item.lang)
                            .eq_ignore_ascii_case(code)
                });
                let Some(translation_item) = translation_item else {
                    continue;
                };
                let translation_count = visible_items
                    .iter()
                    .filter(|i| i.kind == "translation")
                    .count();
                let can_show = preserve
                    && show_translations
                    && translation_count < max_translation_languages as usize
                    && translation_item.success
                    && !translation_item.text.is_empty();
                if !preserve && !translation_item.text.is_empty() {
                    (self.stale_translation_suppressed)(1);
                }
                let updated = SubtitleLineItem {
                    visible: can_show,
                    style_slot: if can_show { Some(code.clone()) } else { None },
                    ..translation_item.clone()
                };
                items.push(updated.clone());
                if updated.visible && !updated.text.is_empty() {
                    visible_items.push(updated);
                }
            }
            payload.sequence = active_partial_sequence.unwrap_or(payload.sequence);
            payload.source_text = active_partial_text.clone();
            payload.source_lang = active_source_lang;
            payload.provider = source_item.provider;
            payload.display_order = display_order;
            payload.show_source = show_source;
            payload.show_translations = show_translations;
            payload.max_translation_languages = max_translation_languages;
            payload.items = items;
            payload.visible_items = visible_items.clone();
            payload.line1 = visible_items
                .first()
                .map(|i| i.text.clone())
                .unwrap_or_default();
            payload.line2 = if visible_items.len() > 1 {
                visible_items[1..]
                    .iter()
                    .map(|i| i.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                String::new()
            };
        }

        let completed_block_visible = completed_payload
            .as_ref()
            .map(|p| !p.visible_items.is_empty())
            .unwrap_or(false);
        payload.lifecycle_state = lifecycle_state;
        payload.completed_sequence = core.completed_sequence();
        payload.completed_block_visible = completed_block_visible;
        payload.completed_expires_at_utc = core.completed_expires_at_utc();
        payload.active_partial_text = visible_partial_text;
        payload.active_partial_sequence = active_partial_sequence;
        payload.active_partial_source_lang = active_partial_source_lang;
        self.log.presentation_built(
            payload.lifecycle_state,
            !active_partial_text.is_empty(),
            completed_payload.is_some(),
            &payload,
        );
        payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn enabled_slot_ids_follow_lines_array_order() {
        let translation = json!({
            "lines": [
                {"enabled": true, "slot_id": "translation_2", "target_lang": "ja"},
                {"enabled": true, "slot_id": "translation_1", "target_lang": "en"},
                {"enabled": false, "slot_id": "translation_3", "target_lang": "ko"},
            ]
        });
        assert_eq!(
            SubtitlePresentation::enabled_slot_ids_in_order(&translation),
            vec!["translation_2".to_string(), "translation_1".to_string()]
        );
    }

    #[test]
    fn resolved_display_order_defaults_follow_lines_array_not_hashmap() {
        let translation = json!({
            "lines": [
                {"enabled": true, "slot_id": "translation_2", "target_lang": "ja"},
                {"enabled": true, "slot_id": "translation_1", "target_lang": "en"},
            ]
        });
        let subtitle_output = json!({});
        assert_eq!(
            SubtitlePresentation::resolved_display_order(&translation, &subtitle_output),
            vec![
                "source".to_string(),
                "translation_2".to_string(),
                "translation_1".to_string()
            ]
        );
    }
}
