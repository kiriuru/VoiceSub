use std::collections::HashMap;
use std::time::Instant;

use crate::policy::{should_emit_partial, PartialEmitInput};
use crate::settings::PartialEmitSettings;

#[derive(Debug, Default)]
pub struct PartialEmitCoordinator {
    last_partial_text: HashMap<String, String>,
    last_partial_emit: HashMap<String, Instant>,
}

impl PartialEmitCoordinator {
    pub fn reset(&mut self) {
        self.last_partial_text.clear();
        self.last_partial_emit.clear();
    }

    pub fn clear_segment(&mut self, segment_id: &str) {
        self.last_partial_text.remove(segment_id);
        self.last_partial_emit.remove(segment_id);
    }

    pub fn should_emit(
        &mut self,
        settings: &PartialEmitSettings,
        segment_id: &str,
        text: &str,
    ) -> bool {
        let previous = self
            .last_partial_text
            .get(segment_id)
            .map(String::as_str)
            .unwrap_or("");
        let should = should_emit_partial(PartialEmitInput {
            new_text: text,
            previous_text: previous,
            mode: &settings.partial_emit_mode,
            min_new_words: settings.partial_min_new_words,
            min_delta_chars: settings.partial_min_delta_chars,
            coalescing_ms: settings.partial_coalescing_ms,
            previous_emit: self.last_partial_emit.get(segment_id).copied(),
            now: Instant::now(),
        });
        if should {
            self.last_partial_text
                .insert(segment_id.to_string(), text.to_string());
            self.last_partial_emit
                .insert(segment_id.to_string(), Instant::now());
        }
        should
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
