use std::sync::Mutex;
use std::time::Instant;

use serde_json::{json, Value};

use crate::trace::SubtitleLog;
use crate::types::SubtitlePayloadEvent;

pub type OverlayBroadcastFn = std::sync::Arc<dyn Fn(Value) + Send + Sync>;

struct Clocks {
    monotonic: Box<dyn Fn() -> Instant + Send + Sync>,
    wall_clock_ms: Box<dyn Fn() -> u64 + Send + Sync>,
}

/// Port of SST `OverlayBroadcaster` — time-dedup for stable overlay frames.
pub struct OverlayBroadcaster {
    broadcast: OverlayBroadcastFn,
    last_payload_signature: Mutex<Option<String>>,
    last_publish_monotonic: Mutex<Option<Instant>>,
    clocks: Clocks,
    log: SubtitleLog,
}

impl OverlayBroadcaster {
    pub fn new(broadcast: OverlayBroadcastFn, log: SubtitleLog) -> Self {
        Self::with_clocks(
            broadcast,
            log,
            Box::new(Instant::now),
            Box::new(|| {
                use std::time::{SystemTime, UNIX_EPOCH};
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_millis() as u64)
                    .unwrap_or(0)
            }),
        )
    }

    pub fn with_clocks(
        broadcast: OverlayBroadcastFn,
        log: SubtitleLog,
        monotonic: Box<dyn Fn() -> Instant + Send + Sync>,
        wall_clock_ms: Box<dyn Fn() -> u64 + Send + Sync>,
    ) -> Self {
        Self {
            broadcast,
            last_payload_signature: Mutex::new(None),
            last_publish_monotonic: Mutex::new(None),
            clocks: Clocks {
                monotonic,
                wall_clock_ms,
            },
            log,
        }
    }

    /// Returns `true` when an overlay frame was broadcast.
    pub fn publish(&self, payload: &SubtitlePayloadEvent) -> bool {
        let mut body = serde_json::to_value(payload).unwrap_or_else(|_| json!({}));
        if let Some(obj) = body.as_object_mut() {
            obj.entry("event_type")
                .or_insert_with(|| json!("overlay_update"));
        }

        let payload_signature = dedupe_signature(&body);
        body["created_at_ms"] = json!((self.clocks.wall_clock_ms)());
        let now_monotonic = (self.clocks.monotonic)();
        let lifecycle_state = body
            .get("lifecycle_state")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let skip_time_dedupe =
            lifecycle_state == "partial_only" || lifecycle_state == "completed_with_partial";
        let signature_dedupe_cooldown_s = if lifecycle_state == "completed_only" {
            0.45
        } else {
            1.0
        };

        let last_signature = self
            .last_payload_signature
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let last_publish = *self
            .last_publish_monotonic
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if !skip_time_dedupe
            && last_signature.as_deref() == Some(payload_signature.as_str())
            && last_publish
                .map(|previous| {
                    now_monotonic.duration_since(previous).as_secs_f64()
                        < signature_dedupe_cooldown_s
                })
                .unwrap_or(false)
        {
            self.log.overlay_publish(
                false,
                payload,
                &format!(
                    "signature_dedupe lifecycle={lifecycle_state} cooldown_s={signature_dedupe_cooldown_s}"
                ),
            );
            return false;
        }

        *self
            .last_payload_signature
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(payload_signature);
        *self
            .last_publish_monotonic
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(now_monotonic);
        (self.broadcast)(json!({
            "type": "overlay_update",
            "payload": body,
        }));
        self.log.overlay_publish(true, payload, "broadcast");
        true
    }
}

fn dedupe_signature(body: &Value) -> String {
    let mut normalized = body.clone();
    if let Some(obj) = normalized.as_object_mut() {
        obj.remove("created_at_ms");
        obj.remove("event_type");
    }
    sorted_json_string(&normalized)
}

fn sorted_json_string(value: &Value) -> String {
    serde_json::to_string(&sort_json_value(value)).unwrap_or_default()
}

fn sort_json_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            let mut sorted = serde_json::Map::new();
            for key in keys {
                sorted.insert(key.clone(), sort_json_value(&map[&key]));
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.iter().map(sort_json_value).collect()),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    use crate::types::LifecycleState;

    fn sample_payload(lifecycle_state: LifecycleState) -> SubtitlePayloadEvent {
        SubtitlePayloadEvent {
            sequence: 1,
            lifecycle_state,
            active_partial_text: "hello".into(),
            show_source: true,
            ..Default::default()
        }
    }

    #[test]
    fn partial_only_payload_not_time_deduped() {
        let messages = Arc::new(Mutex::new(Vec::new()));
        let messages_cb = messages.clone();
        let broadcaster = OverlayBroadcaster::new(
            Arc::new(move |message| {
                messages_cb.lock().unwrap().push(message);
            }),
            SubtitleLog::default(),
        );

        let payload = sample_payload(LifecycleState::PartialOnly);
        assert!(broadcaster.publish(&payload));
        assert!(broadcaster.publish(&payload));
        assert_eq!(messages.lock().unwrap().len(), 2);
    }

    #[test]
    fn completed_only_payload_can_be_deduped() {
        let messages = Arc::new(Mutex::new(Vec::new()));
        let messages_cb = messages.clone();
        let tick = Arc::new(AtomicU64::new(0));
        let tick_cb = tick.clone();
        let start = Instant::now();
        let broadcaster = OverlayBroadcaster::with_clocks(
            Arc::new(move |message| {
                messages_cb.lock().unwrap().push(message);
            }),
            SubtitleLog::default(),
            Box::new(move || {
                start + std::time::Duration::from_millis(tick_cb.fetch_add(100, Ordering::SeqCst))
            }),
            Box::new(|| 1_700_000_000_000),
        );

        let mut payload = sample_payload(LifecycleState::CompletedOnly);
        payload.active_partial_text.clear();
        payload.completed_block_visible = true;
        assert!(broadcaster.publish(&payload));
        assert!(!broadcaster.publish(&payload));
        assert_eq!(messages.lock().unwrap().len(), 1);
    }

    #[test]
    fn completed_only_dedupes_even_when_created_at_ms_differs() {
        let messages = Arc::new(Mutex::new(Vec::new()));
        let messages_cb = messages.clone();
        let tick = Arc::new(AtomicU64::new(0));
        let tick_cb = tick.clone();
        let wall_clock = Arc::new(AtomicU64::new(1_700_000_000_000));
        let wall_clock_cb = wall_clock.clone();
        let start = Instant::now();
        let broadcaster = OverlayBroadcaster::with_clocks(
            Arc::new(move |message| {
                messages_cb.lock().unwrap().push(message);
            }),
            SubtitleLog::default(),
            Box::new(move || {
                start + std::time::Duration::from_millis(tick_cb.fetch_add(100, Ordering::SeqCst))
            }),
            Box::new(move || wall_clock_cb.fetch_add(1, Ordering::SeqCst)),
        );

        let mut payload = sample_payload(LifecycleState::CompletedOnly);
        payload.active_partial_text.clear();
        payload.completed_block_visible = true;
        payload.created_at_ms = Some(111);
        assert!(broadcaster.publish(&payload));
        payload.created_at_ms = Some(222);
        assert!(!broadcaster.publish(&payload));
        assert_eq!(messages.lock().unwrap().len(), 1);
    }

    #[test]
    fn completed_only_payload_can_repeat_after_cooldown() {
        let messages = Arc::new(Mutex::new(Vec::new()));
        let messages_cb = messages.clone();
        let tick = Arc::new(AtomicU64::new(0));
        let tick_cb = tick.clone();
        let start = Instant::now();
        let broadcaster = OverlayBroadcaster::with_clocks(
            Arc::new(move |message| {
                messages_cb.lock().unwrap().push(message);
            }),
            SubtitleLog::default(),
            Box::new(move || {
                start + std::time::Duration::from_millis(tick_cb.fetch_add(500, Ordering::SeqCst))
            }),
            Box::new(|| 1_700_000_000_000),
        );

        let mut payload = sample_payload(LifecycleState::CompletedOnly);
        payload.active_partial_text.clear();
        payload.completed_block_visible = true;
        assert!(broadcaster.publish(&payload));
        assert!(broadcaster.publish(&payload));
        assert_eq!(messages.lock().unwrap().len(), 2);
    }
}
