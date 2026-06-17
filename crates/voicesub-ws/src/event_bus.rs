use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use tokio::sync::broadcast;

const BUS_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct EventBusDiagnostics {
    pub subscriber_count: usize,
    pub revision: u64,
    pub publish_count: u64,
    pub channel_capacity: usize,
}

/// In-process event bus for desktop shell clients (dashboard/TTS).
/// WebSocket transport remains for OBS overlay and browser worker.
#[derive(Clone)]
pub struct RuntimeEventBus {
    tx: broadcast::Sender<Arc<Value>>,
    rev: Arc<AtomicU64>,
    publish_count: Arc<AtomicU64>,
}

impl Default for RuntimeEventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeEventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BUS_CAPACITY);
        Self {
            tx,
            rev: Arc::new(AtomicU64::new(0)),
            publish_count: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn publish(&self, message: Value) {
        self.rev.fetch_add(1, Ordering::Relaxed);
        self.publish_count.fetch_add(1, Ordering::Relaxed);
        let _ = self.tx.send(Arc::new(message));
    }

    pub fn revision(&self) -> u64 {
        self.rev.load(Ordering::Relaxed)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Value>> {
        self.tx.subscribe()
    }

    pub fn diagnostics(&self) -> EventBusDiagnostics {
        EventBusDiagnostics {
            subscriber_count: self.tx.receiver_count(),
            revision: self.revision(),
            publish_count: self.publish_count.load(Ordering::Relaxed),
            channel_capacity: BUS_CAPACITY,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RuntimeStateSnapshot {
    pub rev: u64,
    pub runtime: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Value>,
}

impl RuntimeStateSnapshot {
    pub fn empty(rev: u64) -> Self {
        Self {
            rev,
            runtime: Value::Null,
            subtitle: None,
            overlay: None,
            translation: None,
            diagnostics: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn bus_delivers_published_messages() {
        let bus = RuntimeEventBus::new();
        let mut rx = bus.subscribe();
        bus.publish(json!({"type": "runtime_update", "payload": {"running": true}}));
        let message = rx.recv().await.expect("bus message");
        assert_eq!(message["type"], "runtime_update");
        assert!(bus.revision() >= 1);
        let diag = bus.diagnostics();
        assert_eq!(diag.publish_count, 1);
        assert!(diag.subscriber_count >= 1);
    }
}
