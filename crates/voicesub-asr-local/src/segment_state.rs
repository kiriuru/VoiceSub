//! SST `segment_state_controller.py` — segment id + revision tracking for local ASR.

use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Default)]
pub struct SegmentStateController {
    sequence: u64,
    segment_counter: u64,
    active_segment_id: Option<String>,
    active_segment_revision: u64,
    last_partial_text_by_segment: HashMap<String, String>,
    last_partial_emit_monotonic_by_segment: HashMap<String, Instant>,
}

impl SegmentStateController {
    pub fn active_segment_id(&self) -> Option<&str> {
        self.active_segment_id.as_deref()
    }

    pub fn active_segment_revision(&self) -> u64 {
        self.active_segment_revision
    }

    pub fn clear_active_segment(&mut self) {
        self.active_segment_id = None;
        self.active_segment_revision = 0;
    }

    pub fn clear_partial_tracking_for_segment(&mut self, segment_id: Option<&str>) {
        let Some(segment_id) = segment_id.filter(|id| !id.trim().is_empty()) else {
            return;
        };
        self.last_partial_text_by_segment.remove(segment_id);
        self.last_partial_emit_monotonic_by_segment.remove(segment_id);
    }

    pub fn mark_partial_emitted(&mut self, segment_id: &str, text: &str) {
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        self.last_partial_text_by_segment
            .insert(segment_id.to_string(), normalized);
        self.last_partial_emit_monotonic_by_segment
            .insert(segment_id.to_string(), Instant::now());
    }

    pub fn get_last_partial_text(&self, segment_id: &str) -> &str {
        self.last_partial_text_by_segment
            .get(segment_id)
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn last_partial_emit_at(&self, segment_id: &str) -> Option<Instant> {
        self.last_partial_emit_monotonic_by_segment
            .get(segment_id)
            .copied()
    }

    /// Returns `(segment_id, revision, started_now, previous_segment_id_to_clear)`.
    pub fn assign_segment_tracking(
        &mut self,
        preferred_segment_id: Option<&str>,
    ) -> (String, u64, bool, Option<String>) {
        let mut started_now = false;
        let mut previous_to_clear = None;
        let normalized_preferred = preferred_segment_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string);

        if let Some(preferred) = normalized_preferred.as_deref() {
            if self.active_segment_id.as_deref() != Some(preferred) {
                previous_to_clear = self.active_segment_id.clone();
                self.active_segment_id = Some(preferred.to_string());
                self.active_segment_revision = 0;
                started_now = true;
            }
        } else if self.active_segment_id.is_none() {
            self.segment_counter += 1;
            self.active_segment_id = Some(format!("segment-{}", self.segment_counter));
            self.active_segment_revision = 0;
            started_now = true;
        }

        self.active_segment_revision += 1;
        (
            self.active_segment_id.clone().unwrap_or_default(),
            self.active_segment_revision,
            started_now,
            previous_to_clear,
        )
    }

    pub fn reset(&mut self) {
        self.sequence = 0;
        self.segment_counter = 0;
        self.clear_active_segment();
        self.last_partial_text_by_segment.clear();
        self.last_partial_emit_monotonic_by_segment.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assign_segment_tracking_bumps_revision() {
        let mut state = SegmentStateController::default();
        let (id, rev, started, _) = state.assign_segment_tracking(None);
        assert!(started);
        assert_eq!(rev, 1);
        let (_, rev2, started2, _) = state.assign_segment_tracking(Some(&id));
        assert!(!started2);
        assert_eq!(rev2, 2);
    }
}
