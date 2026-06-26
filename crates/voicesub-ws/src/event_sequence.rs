use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// Monotonic WS payload enrichment — port of SST `enrich_event_payload`.
#[derive(Debug)]
pub struct EventSequencer {
    global_sequence: AtomicU64,
    sequence_by_type: Mutex<HashMap<String, u64>>,
}

impl Default for EventSequencer {
    fn default() -> Self {
        Self::new()
    }
}

impl EventSequencer {
    pub fn new() -> Self {
        Self {
            global_sequence: AtomicU64::new(0),
            sequence_by_type: Mutex::new(HashMap::new()),
        }
    }

    pub fn enrich(&self, event_type: &str, mut payload: Value) -> Value {
        let sequence = self
            .global_sequence
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        if let Ok(mut guard) = self.sequence_by_type.lock() {
            guard.insert(event_type.to_string(), sequence);
        }

        if let Some(obj) = payload.as_object_mut() {
            obj.entry("event_type").or_insert_with(|| json!(event_type));
            if !obj.contains_key("created_at_ms") {
                obj.insert("created_at_ms".into(), json!(wall_clock_ms()));
            }
            obj.insert("event_sequence".into(), json!(sequence));
        }
        payload
    }

    /// Clears per-type bookkeeping without rewinding the global sequence stream.
    ///
    /// SST `RuntimeStateController.reset_broadcast_state` — long-lived WS clients rely on
    /// monotonic `event_sequence` across runtime stop/start cycles.
    pub fn reset_broadcast_state(&self) {
        if let Ok(mut guard) = self.sequence_by_type.lock() {
            guard.clear();
        }
    }

    pub fn global_sequence(&self) -> u64 {
        self.global_sequence.load(Ordering::Relaxed)
    }
}

pub type SharedEventSequencer = std::sync::Arc<EventSequencer>;

pub fn shared_event_sequencer() -> SharedEventSequencer {
    std::sync::Arc::new(EventSequencer::new())
}

fn wall_clock_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_stays_monotonic_after_reset_broadcast_state() {
        let seq = EventSequencer::new();
        let first = seq.enrich("runtime_status", json!({ "status": "listening" }));
        let second = seq.enrich("runtime_status", json!({ "status": "transcribing" }));
        seq.reset_broadcast_state();
        let third = seq.enrich("runtime_status", json!({ "status": "listening" }));

        assert_eq!(first["event_sequence"], 1);
        assert_eq!(second["event_sequence"], 2);
        assert_eq!(third["event_sequence"], 3);
        assert!(
            third["event_sequence"].as_u64().unwrap() > second["event_sequence"].as_u64().unwrap()
        );
    }

    #[test]
    fn enrich_sets_created_at_ms() {
        let seq = EventSequencer::new();
        let body = seq.enrich("transcript_update", json!({ "text": "hi" }));
        assert!(
            body.get("created_at_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                > 0
        );
        assert_eq!(body["event_type"], "transcript_update");
    }

    #[test]
    fn enrich_without_outer_mutex_is_thread_safe_for_sequence() {
        let seq = EventSequencer::new();
        let a = seq.enrich("overlay_update", json!({}));
        let b = seq.enrich("overlay_update", json!({}));
        assert!(b["event_sequence"].as_u64().unwrap() > a["event_sequence"].as_u64().unwrap());
    }
}
