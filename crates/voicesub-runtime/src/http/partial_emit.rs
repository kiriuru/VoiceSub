//! Browser/runtime partial emit — delegates policy to `voicesub-partial-emit`.

pub use voicesub_partial_emit::{
    partial_emit_settings_from_config, should_emit_partial, PartialEmitInput, PartialEmitSettings,
};

use std::time::Instant;

use crate::segment_state::SegmentStateController;

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
