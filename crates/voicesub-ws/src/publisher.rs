use serde_json::{json, Value};

use std::sync::{mpsc, OnceLock};

use crate::event_sequence::SharedEventSequencer;
use crate::events::EventsHub;

/// Wraps [`EventsHub`] with SST-compatible payload enrichment.
#[derive(Clone)]
pub struct WsEventPublisher {
    hub: EventsHub,
    sequencer: SharedEventSequencer,
}

impl WsEventPublisher {
    pub fn new(hub: EventsHub, sequencer: SharedEventSequencer) -> Self {
        Self { hub, sequencer }
    }

    pub fn hub(&self) -> EventsHub {
        self.hub.clone()
    }

    pub fn sequencer(&self) -> SharedEventSequencer {
        self.sequencer.clone()
    }

    pub fn reset_broadcast_state(&self) {
        if let Ok(mut guard) = self.sequencer.lock() {
            guard.reset_broadcast_state();
        }
    }

    pub async fn broadcast_channel(&self, channel: &str, enrich_as: &str, payload: Value) {
        let enriched = self.enrich_payload(enrich_as, payload);
        self.hub
            .broadcast(json!({
                "type": channel,
                "payload": enriched,
            }))
            .await;
    }

    pub fn broadcast_channel_now(&self, channel: &str, enrich_as: &str, payload: Value) {
        let enriched = self.enrich_payload(enrich_as, payload);
        let message = json!({
            "type": channel,
            "payload": enriched,
        });
        broadcast_now(&self.hub, message);
    }

    /// Overlay/subtitle payloads are already shaped; enrich in-place for stale guards.
    pub async fn broadcast_overlay_body(&self, channel: &str, enrich_as: &str, mut body: Value) {
        let enriched = self.enrich_payload(enrich_as, body.take());
        self.hub
            .broadcast(json!({
                "type": channel,
                "payload": enriched,
            }))
            .await;
    }

    pub fn broadcast_overlay_body_now(&self, channel: &str, enrich_as: &str, mut body: Value) {
        let enriched = self.enrich_payload(enrich_as, body.take());
        broadcast_now(
            &self.hub,
            json!({
                "type": channel,
                "payload": enriched,
            }),
        );
    }

    fn enrich_payload(&self, enrich_as: &str, payload: Value) -> Value {
        match self.sequencer.lock() {
            Ok(mut guard) => guard.enrich(enrich_as, payload),
            Err(_) => payload,
        }
    }
}

struct SyncBroadcastJob {
    hub: EventsHub,
    message: Value,
}

fn sync_broadcast_loop(rx: mpsc::Receiver<SyncBroadcastJob>) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("ws publisher broadcast runtime");
    for job in rx {
        rt.block_on(job.hub.broadcast(job.message));
    }
}

static SYNC_BROADCAST_TX: OnceLock<mpsc::Sender<SyncBroadcastJob>> = OnceLock::new();

fn sync_broadcast_sender() -> &'static mpsc::Sender<SyncBroadcastJob> {
    SYNC_BROADCAST_TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("voicesub-ws-sync-broadcast".into())
            .spawn(move || sync_broadcast_loop(rx))
            .expect("spawn sync broadcast thread");
        tx
    })
}

fn broadcast_now(events: &EventsHub, message: Value) {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        let events = events.clone();
        handle.spawn(async move {
            events.broadcast(message).await;
        });
        return;
    }
    let tx = sync_broadcast_sender();
    if tx
        .send(SyncBroadcastJob {
            hub: events.clone(),
            message,
        })
        .is_err()
    {
        tracing::warn!("sync ws broadcast channel closed; dropping message");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::event_sequence::EventSequencer;

    #[test]
    fn enrich_as_can_differ_from_channel() {
        let publisher = WsEventPublisher::new(
            EventsHub::new(),
            Arc::new(Mutex::new(EventSequencer::default())),
        );
        let body = publisher
            .sequencer()
            .lock()
            .unwrap()
            .enrich("runtime_status", json!({ "running": true }));
        assert_eq!(body["event_type"], "runtime_status");
    }
}
