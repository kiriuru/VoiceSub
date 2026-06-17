//! Port of SST `SegmentStateController` — segment/revision/sequence tracking for browser ASR.

use std::time::Instant;

#[derive(Debug, Default)]
pub struct SegmentStateController {
    sequence: u64,
    segment_counter: u64,
    active_segment_id: Option<String>,
    active_segment_revision: u64,
    last_partial_text_by_segment: std::collections::HashMap<String, String>,
    last_partial_emit_monotonic_by_segment: std::collections::HashMap<String, Instant>,
}

impl SegmentStateController {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn active_segment_id(&self) -> Option<&str> {
        self.active_segment_id.as_deref()
    }

    pub fn active_segment_revision(&self) -> u64 {
        self.active_segment_revision
    }

    pub fn reset_sequence(&mut self) {
        self.sequence = 0;
    }

    pub fn next_sequence(&mut self) -> u64 {
        self.sequence += 1;
        self.sequence
    }

    pub fn reset_segment_counter(&mut self) {
        self.segment_counter = 0;
    }

    fn next_segment_id(&mut self, prefix: &str) -> String {
        self.segment_counter += 1;
        format!("{prefix}-{}", self.segment_counter)
    }

    pub fn clear_active_segment(&mut self) {
        self.active_segment_id = None;
        self.active_segment_revision = 0;
    }

    pub fn bump_active_segment_revision(&mut self) -> u64 {
        self.active_segment_revision += 1;
        self.active_segment_revision
    }

    pub fn clear_all_partial_tracking(&mut self) {
        self.last_partial_text_by_segment.clear();
        self.last_partial_emit_monotonic_by_segment.clear();
    }

    pub fn clear_partial_tracking_for_segment(&mut self, segment_id: Option<&str>) {
        let Some(segment_id) = segment_id.filter(|id| !id.trim().is_empty()) else {
            return;
        };
        self.last_partial_text_by_segment.remove(segment_id);
        self.last_partial_emit_monotonic_by_segment
            .remove(segment_id);
    }

    pub fn get_last_partial_text(&self, segment_id: &str) -> &str {
        self.last_partial_text_by_segment
            .get(segment_id)
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn get_last_partial_emit_monotonic(&self, segment_id: &str) -> Option<Instant> {
        self.last_partial_emit_monotonic_by_segment
            .get(segment_id)
            .copied()
    }

    pub fn mark_partial_emitted(&mut self, segment_id: &str, text: &str) {
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        self.last_partial_text_by_segment
            .insert(segment_id.to_string(), normalized);
        self.last_partial_emit_monotonic_by_segment
            .insert(segment_id.to_string(), Instant::now());
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
            self.active_segment_id = Some(self.next_segment_id("segment"));
            self.active_segment_revision = 0;
            started_now = true;
        }

        let revision = self.bump_active_segment_revision();
        (
            self.active_segment_id.clone().unwrap_or_default(),
            revision,
            started_now,
            previous_to_clear,
        )
    }

    pub fn cleanup_on_browser_worker_disconnect(&mut self) {
        let segment_id = self.active_segment_id.clone();
        self.clear_active_segment();
        self.clear_partial_tracking_for_segment(segment_id.as_deref());
    }

    pub fn reset(&mut self) {
        self.reset_sequence();
        self.reset_segment_counter();
        self.clear_active_segment();
        self.clear_all_partial_tracking();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assign_segment_tracking_reuses_worker_segment_id() {
        let mut state = SegmentStateController::default();
        let (id, rev, started, _) = state.assign_segment_tracking(Some("worker-g0-s1"));
        assert_eq!(id, "worker-g0-s1");
        assert_eq!(rev, 1);
        assert!(started);

        let (_, rev2, started2, _) = state.assign_segment_tracking(Some("worker-g0-s1"));
        assert_eq!(rev2, 2);
        assert!(!started2);
    }

    #[test]
    fn next_sequence_increments_globally() {
        let mut state = SegmentStateController::default();
        assert_eq!(state.next_sequence(), 1);
        assert_eq!(state.next_sequence(), 2);
    }
}
