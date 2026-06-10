use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

/// Monotonic WS payload enrichment — port of SST `enrich_event_payload`.
#[derive(Debug, Default)]
pub struct EventSequencer {
    global_sequence: u64,
    sequence_by_type: HashMap<String, u64>,
}

impl EventSequencer {
    pub fn enrich(&mut self, event_type: &str, mut payload: Value) -> Value {
        self.global_sequence = self.global_sequence.saturating_add(1);
        let sequence = self.global_sequence;
        self.sequence_by_type
            .insert(event_type.to_string(), sequence);

        if let Some(obj) = payload.as_object_mut() {
            obj.entry("event_type")
                .or_insert_with(|| json!(event_type));
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
    pub fn reset_broadcast_state(&mut self) {
        self.sequence_by_type.clear();
    }

    pub fn global_sequence(&self) -> u64 {
        self.global_sequence
    }
}

pub type SharedEventSequencer = Arc<Mutex<EventSequencer>>;

pub fn shared_event_sequencer() -> SharedEventSequencer {
    Arc::new(Mutex::new(EventSequencer::default()))
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
        let mut seq = EventSequencer::default();
        let first = seq.enrich("runtime_status", json!({ "status": "listening" }));
        let second = seq.enrich("runtime_status", json!({ "status": "transcribing" }));
        seq.reset_broadcast_state();
        let third = seq.enrich("runtime_status", json!({ "status": "listening" }));

        assert_eq!(first["event_sequence"], 1);
        assert_eq!(second["event_sequence"], 2);
        assert_eq!(third["event_sequence"], 3);
        assert!(third["event_sequence"].as_u64().unwrap() > second["event_sequence"].as_u64().unwrap());
    }

    #[test]
    fn enrich_sets_created_at_ms() {
        let mut seq = EventSequencer::default();
        let body = seq.enrich("transcript_update", json!({ "text": "hi" }));
        assert!(body.get("created_at_ms").and_then(|v| v.as_u64()).unwrap_or(0) > 0);
        assert_eq!(body["event_type"], "transcript_update");
    }
}
