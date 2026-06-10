use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechQueueItem {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub source: String,
    /// BCP-47 / ISO language code for browser TTS (e.g. `ru`, `en`).
    #[serde(default = "default_speech_lang")]
    pub lang: String,
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

    pub fn enqueue(&mut self, item: SpeechQueueItem) {
        self.enqueue_with_cap(item, 8);
    }

    pub fn enqueue_with_cap(&mut self, item: SpeechQueueItem, max_items: u32) {
        let cap = max_items.max(1) as usize;
        let dropped = if self.items.len() >= cap {
            self.items.pop_front().map(|dropped| dropped.id)
        } else {
            None
        };
        debug!(
            target: "voicesub.tts",
            id = %item.id,
            queue_len = self.items.len() + 1,
            dropped_id = dropped.as_deref().unwrap_or(""),
            cap,
            "queue push"
        );
        self.items.push_back(item);
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

    pub fn mark_finished(&mut self, item_id: &str) {
        if self.current_id.as_deref() == Some(item_id) {
            self.current_id = None;
            self.state = SpeechQueueState::Idle;
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, text: &str) -> SpeechQueueItem {
        SpeechQueueItem {
            id: id.to_string(),
            text: text.to_string(),
            source: String::new(),
            lang: "en".to_string(),
        }
    }

    #[test]
    fn fifo_order() {
        let mut q = SpeechQueue::new();
        q.enqueue(item("a", "one"));
        q.enqueue(item("b", "two"));
        assert_eq!(q.begin_next().unwrap().id, "a");
        q.mark_finished("a");
        assert_eq!(q.begin_next().unwrap().id, "b");
    }

    #[test]
    fn enqueue_with_cap_drops_oldest() {
        let mut q = SpeechQueue::new();
        for index in 0..10 {
            q.enqueue_with_cap(item(&format!("id-{index}"), "x"), 8);
        }
        assert_eq!(q.len(), 8);
        assert_eq!(q.begin_next().unwrap().id, "id-2");
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
}
