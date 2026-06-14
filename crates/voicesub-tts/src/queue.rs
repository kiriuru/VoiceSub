use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechQueueItem {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub source: String,
    /// BCP-47 / ISO language code for browser TTS (e.g. `ru`, `en`).
    #[serde(default = "default_speech_lang")]
    pub lang: String,
    /// Subtitle planner dedupe key; released when this item is dropped before playback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dedupe_key: Option<String>,
}

/// Result of enqueueing into a speech channel (includes adaptive-drop ids for JS prefetch cleanup).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelEnqueueResult {
    pub queue_len: usize,
    #[serde(default)]
    pub dropped_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkFinishedOutcome {
    Matched,
    NotSpeaking,
    MismatchForcedIdle,
}

fn default_speech_lang() -> String {
    "en".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpeechQueueState {
    #[default]
    Idle,
    Speaking,
    Paused,
}

/// In-memory FIFO queue used by the TTS window (Phase 1 prototype).
#[derive(Debug)]
pub struct SpeechQueue {
    items: VecDeque<SpeechQueueItem>,
    state: SpeechQueueState,
    current_id: Option<String>,
}

impl Default for SpeechQueue {
    fn default() -> Self {
        Self {
            items: VecDeque::new(),
            state: SpeechQueueState::Idle,
            current_id: None,
        }
    }
}

impl SpeechQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> SpeechQueueState {
        self.state
    }

    pub fn current_id(&self) -> Option<&str> {
        self.current_id.as_deref()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn snapshot(&self) -> Vec<SpeechQueueItem> {
        self.items.iter().cloned().collect()
    }

    pub fn enqueue(&mut self, item: SpeechQueueItem) -> Vec<SpeechQueueItem> {
        self.enqueue_with_cap(item, 8)
    }

    /// Returns items removed by adaptive drop before enqueueing `item`.
    pub fn enqueue_with_cap(&mut self, item: SpeechQueueItem, max_items: u32) -> Vec<SpeechQueueItem> {
        let cap = max_items.max(1) as usize;
        let dropped = adaptive_drop_for_enqueue(&mut self.items, cap);
        debug!(
            target: "voicesub.tts",
            id = %item.id,
            queue_len = self.items.len() + 1,
            dropped_count = dropped.len(),
            dropped_ids = %dropped
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>()
                .join(","),
            cap,
            low_water = adaptive_low_water(cap),
            "queue push"
        );
        self.items.push_back(item);
        dropped
    }

    pub fn clear(&mut self) {
        let dropped = self.items.len();
        self.items.clear();
        self.current_id = None;
        self.state = SpeechQueueState::Idle;
        debug!(target: "voicesub.tts", dropped, "queue cleared");
    }

    pub fn begin_next(&mut self) -> Option<SpeechQueueItem> {
        if self.state == SpeechQueueState::Speaking {
            return None;
        }
        let next = self.items.pop_front()?;
        self.current_id = Some(next.id.clone());
        self.state = SpeechQueueState::Speaking;
        Some(next)
    }

    pub fn mark_finished(&mut self, item_id: &str) -> MarkFinishedOutcome {
        if self.state != SpeechQueueState::Speaking {
            return MarkFinishedOutcome::NotSpeaking;
        }
        if self.current_id.as_deref() == Some(item_id) {
            self.current_id = None;
            self.state = SpeechQueueState::Idle;
            return MarkFinishedOutcome::Matched;
        }
        let expected = self.current_id.clone().unwrap_or_default();
        warn!(
            target: "voicesub.tts",
            expected = %expected,
            got = %item_id,
            "queue mark_finished id mismatch; forcing idle"
        );
        self.force_idle();
        MarkFinishedOutcome::MismatchForcedIdle
    }

    pub fn pause(&mut self) {
        if self.state == SpeechQueueState::Speaking {
            self.state = SpeechQueueState::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.state == SpeechQueueState::Paused {
            self.state = SpeechQueueState::Speaking;
        }
    }

    /// Reset a stuck `Speaking` state without dropping queued items.
    pub fn force_idle(&mut self) {
        self.current_id = None;
        self.state = SpeechQueueState::Idle;
    }
}

fn adaptive_low_water(cap: usize) -> usize {
    (cap / 2).max(1)
}

fn pick_adaptive_drop_index(_items: &VecDeque<SpeechQueueItem>) -> usize {
    // Drop the oldest queued item first so backlog trimming stays fair across source and translations.
    0
}

fn adaptive_drop_for_enqueue(
    items: &mut VecDeque<SpeechQueueItem>,
    cap: usize,
) -> Vec<SpeechQueueItem> {
    let low_water = adaptive_low_water(cap);
    let mut dropped = Vec::new();
    if items.len() < cap {
        return dropped;
    }
    while items.len() > low_water {
        let index = pick_adaptive_drop_index(items);
        if let Some(removed) = items.remove(index) {
            dropped.push(removed);
        } else {
            break;
        }
    }
    dropped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, text: &str) -> SpeechQueueItem {
        item_with_source(id, text, "")
    }

    fn item_with_source(id: &str, text: &str, source: &str) -> SpeechQueueItem {
        SpeechQueueItem {
            id: id.to_string(),
            text: text.to_string(),
            source: source.to_string(),
            lang: "en".to_string(),
            dedupe_key: None,
        }
    }

    #[test]
    fn fifo_order() {
        let mut q = SpeechQueue::new();
        q.enqueue(item("a", "one"));
        q.enqueue(item("b", "two"));
        assert_eq!(q.begin_next().unwrap().id, "a");
        assert_eq!(q.mark_finished("a"), MarkFinishedOutcome::Matched);
        assert_eq!(q.begin_next().unwrap().id, "b");
    }

    #[test]
    fn enqueue_with_cap_trims_to_low_water_when_saturated() {
        let mut q = SpeechQueue::new();
        for index in 0..10 {
            q.enqueue_with_cap(item(&format!("id-{index}"), "x"), 8);
        }
        assert_eq!(q.len(), 6);
        assert_eq!(q.begin_next().unwrap().id, "id-4");
    }

    #[test]
    fn adaptive_drop_removes_oldest_items_first() {
        let mut items = VecDeque::new();
        for index in 0..4 {
            items.push_back(item_with_source(
                &format!("src-{index}"),
                "source",
                "subtitle_source",
            ));
        }
        for index in 0..4 {
            items.push_back(item_with_source(
                &format!("tl-{index}"),
                "translation",
                "subtitle_translation_1",
            ));
        }
        let dropped = adaptive_drop_for_enqueue(&mut items, 8);
        assert_eq!(dropped.len(), 4);
        assert!(dropped.iter().all(|item| item.id.starts_with("src-")));
        assert_eq!(items.len(), 4);
        assert!(items.iter().all(|item| item.id.starts_with("tl-")));
    }

    #[test]
    fn clear_resets_state() {
        let mut q = SpeechQueue::new();
        q.enqueue(item("a", "one"));
        let _ = q.begin_next();
        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.state(), SpeechQueueState::Idle);
    }

    #[test]
    fn mark_finished_mismatch_forces_idle() {
        let mut q = SpeechQueue::new();
        q.enqueue(item("a", "one"));
        q.enqueue(item("b", "two"));
        let _ = q.begin_next();
        assert_eq!(q.mark_finished("wrong"), MarkFinishedOutcome::MismatchForcedIdle);
        assert_eq!(q.state(), SpeechQueueState::Idle);
        assert_eq!(q.begin_next().unwrap().id, "b");
    }

    #[test]
    fn force_idle_unblocks_stuck_speaking_state() {
        let mut q = SpeechQueue::new();
        q.enqueue(item("a", "one"));
        q.enqueue(item("b", "two"));
        let _ = q.begin_next();
        assert_eq!(q.state(), SpeechQueueState::Speaking);
        q.force_idle();
        assert_eq!(q.state(), SpeechQueueState::Idle);
        assert_eq!(q.begin_next().unwrap().id, "b");
    }

    #[test]
    fn channel_enqueue_result_serializes_empty_dropped_ids() {
        let json = serde_json::to_string(&ChannelEnqueueResult {
            queue_len: 3,
            dropped_ids: vec![],
        })
        .expect("serialize");
        assert!(json.contains("\"dropped_ids\""));
        let parsed: ChannelEnqueueResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.queue_len, 3);
        assert!(parsed.dropped_ids.is_empty());
    }
}
