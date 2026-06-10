use std::time::Instant;

use serde_json::Value;

use crate::segment_state::SegmentStateController;

#[derive(Debug, Clone)]
pub struct PartialEmitSettings {
    pub partial_emit_mode: String,
    pub partial_min_new_words: u32,
    pub partial_min_delta_chars: u32,
    pub partial_coalescing_ms: u32,
}

impl Default for PartialEmitSettings {
    fn default() -> Self {
        Self {
            partial_emit_mode: "word_growth".into(),
            partial_min_new_words: 1,
            partial_min_delta_chars: 0,
            partial_coalescing_ms: 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct PartialEmitCoordinator {
    pub segment_state: SegmentStateController,
}

impl PartialEmitCoordinator {
    pub fn reset(&mut self) {
        self.segment_state.reset();
    }

    pub fn clear_segment(&mut self, segment_id: &str) {
        self.segment_state
            .clear_partial_tracking_for_segment(Some(segment_id));
    }

    pub fn should_emit(
        &mut self,
        settings: &PartialEmitSettings,
        segment_id: &str,
        text: &str,
    ) -> bool {
        let previous = self.segment_state.get_last_partial_text(segment_id);
        let should = should_emit_partial(PartialEmitInput {
            new_text: text,
            previous_text: previous,
            mode: &settings.partial_emit_mode,
            min_new_words: settings.partial_min_new_words,
            min_delta_chars: settings.partial_min_delta_chars,
            coalescing_ms: settings.partial_coalescing_ms,
            previous_emit: self
                .segment_state
                .get_last_partial_emit_monotonic(segment_id),
            now: Instant::now(),
        });
        if should {
            self.segment_state.mark_partial_emitted(segment_id, text);
        }
        should
    }
}

pub fn partial_emit_settings_from_config(config: &Value) -> PartialEmitSettings {
    let realtime = config
        .get("asr")
        .and_then(|v| v.get("realtime"))
        .cloned()
        .unwrap_or(Value::Null);
    let mut settings = PartialEmitSettings::default();
    if let Some(mode) = realtime.get("partial_emit_mode").and_then(|v| v.as_str()) {
        let normalized = mode.trim().to_ascii_lowercase();
        if normalized == "char_delta" || normalized == "word_growth" {
            settings.partial_emit_mode = normalized;
        }
    }
    settings.partial_min_new_words = realtime
        .get("partial_min_new_words")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .clamp(1, 32) as u32;
    settings.partial_min_delta_chars = realtime
        .get("partial_min_delta_chars")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        .clamp(0, 256) as u32;
    settings.partial_coalescing_ms = realtime
        .get("partial_coalescing_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        .clamp(0, 10_000) as u32;
    settings
}

pub fn normalize_transcript_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn split_words(text: &str) -> Vec<String> {
    let normalized = normalize_transcript_text(text);
    if normalized.is_empty() {
        return Vec::new();
    }
    normalized.split(' ').map(str::to_string).collect()
}

#[derive(Debug, Clone, Copy)]
pub struct PartialEmitInput<'a> {
    pub new_text: &'a str,
    pub previous_text: &'a str,
    pub mode: &'a str,
    pub min_new_words: u32,
    pub min_delta_chars: u32,
    pub coalescing_ms: u32,
    pub previous_emit: Option<Instant>,
    pub now: Instant,
}

pub fn should_emit_partial(input: PartialEmitInput<'_>) -> bool {
    let PartialEmitInput {
        new_text,
        previous_text,
        mode,
        min_new_words,
        min_delta_chars,
        coalescing_ms,
        previous_emit,
        now,
    } = input;
    let new_norm = normalize_transcript_text(new_text);
    if new_norm.is_empty() {
        return false;
    }
    let prev_norm = normalize_transcript_text(previous_text);
    if new_norm == prev_norm {
        return false;
    }

    let emit_mode = mode.trim().to_ascii_lowercase();
    if emit_mode == "char_delta" {
        return should_emit_char_delta(
            &new_norm,
            &prev_norm,
            min_delta_chars,
            coalescing_ms,
            previous_emit,
            now,
        );
    }

    should_emit_word_growth(&new_norm, &prev_norm, min_new_words.max(1))
}

fn should_emit_char_delta(
    new_norm: &str,
    prev_norm: &str,
    min_delta_chars: u32,
    coalescing_ms: u32,
    previous_emit: Option<Instant>,
    now: Instant,
) -> bool {
    let growth_chars = new_norm
        .chars()
        .count()
        .saturating_sub(prev_norm.chars().count()) as i64;
    if prev_norm.is_empty() {
        return true;
    }
    if min_delta_chars == 0 && coalescing_ms == 0 {
        return true;
    }
    if growth_chars < 0 {
        return true;
    }
    if min_delta_chars > 0 && growth_chars >= min_delta_chars as i64 {
        return true;
    }
    if coalescing_ms > 0 {
        if let Some(previous_emit) = previous_emit {
            if growth_chars >= 0 && (min_delta_chars == 0 || growth_chars < min_delta_chars as i64)
            {
                let elapsed_ms = now.duration_since(previous_emit).as_millis() as u32;
                return elapsed_ms >= coalescing_ms;
            }
        }
    }
    growth_chars > 0
}

fn should_emit_word_growth(new_norm: &str, prev_norm: &str, min_new_words: u32) -> bool {
    let new_words = split_words(new_norm);
    let prev_words = split_words(prev_norm);

    if prev_words.is_empty() {
        return new_words.len() >= min_new_words as usize;
    }
    if new_words.len() < prev_words.len() {
        return false;
    }
    if new_words.len() == prev_words.len() {
        return new_words != prev_words;
    }
    if new_words[..prev_words.len()] != prev_words[..] {
        return true;
    }
    let added = new_words.len() - prev_words.len();
    added >= min_new_words as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emit_input<'a>(
        new_text: &'a str,
        previous_text: &'a str,
        mode: &'a str,
        min_new_words: u32,
    ) -> PartialEmitInput<'a> {
        PartialEmitInput {
            new_text,
            previous_text,
            mode,
            min_new_words,
            min_delta_chars: 0,
            coalescing_ms: 0,
            previous_emit: None,
            now: Instant::now(),
        }
    }

    #[test]
    fn word_growth_requires_new_words() {
        assert!(!should_emit_partial(emit_input(
            "hello",
            "hello",
            "word_growth",
            1
        )));
        assert!(should_emit_partial(emit_input(
            "hello world",
            "",
            "word_growth",
            1
        )));
        assert!(!should_emit_partial(emit_input(
            "hello there",
            "hello",
            "word_growth",
            2,
        )));
    }

    #[test]
    fn coordinator_uses_word_growth_and_clears_segment() {
        let settings = PartialEmitSettings::default();
        let mut coord = PartialEmitCoordinator::default();
        assert!(coord.should_emit(&settings, "s1", "hello"));
        assert!(!coord.should_emit(&settings, "s1", "hello"));
        assert!(coord.should_emit(&settings, "s1", "hello world"));
        coord.clear_segment("s1");
        assert!(coord.should_emit(&settings, "s1", "hello"));
    }

    #[test]
    fn char_delta_emits_on_growth() {
        let mut input = emit_input("hello", "", "char_delta", 1);
        input.min_delta_chars = 1;
        assert!(should_emit_partial(input));
    }
}
