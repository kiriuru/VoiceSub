use serde_json::{Value, json};

use std::sync::{OnceLock, mpsc};

use crate::event_bus::RuntimeEventBus;
use crate::event_sequence::SharedEventSequencer;
use crate::events::EventsHub;

/// Wraps [`EventsHub`] with SST-compatible payload enrichment.
#[derive(Clone)]
pub struct WsEventPublisher {
    hub: EventsHub,
    sequencer: SharedEventSequencer,
    event_bus: Option<RuntimeEventBus>,
}

impl WsEventPublisher {
    pub fn new(hub: EventsHub, sequencer: SharedEventSequencer) -> Self {
        Self::with_event_bus(hub, sequencer, None)
    }

    pub fn with_event_bus(
        hub: EventsHub,
        sequencer: SharedEventSequencer,
        event_bus: Option<RuntimeEventBus>,
    ) -> Self {
        Self {
            hub,
            sequencer,
            event_bus,
        }
    }

    pub fn event_bus(&self) -> Option<RuntimeEventBus> {
        self.event_bus.clone()
    }

    pub fn hub(&self) -> EventsHub {
        self.hub.clone()
    }

    pub fn sequencer(&self) -> SharedEventSequencer {
        self.sequencer.clone()
    }

    pub fn reset_broadcast_state(&self) {
        self.sequencer.reset_broadcast_state();
    }

    pub async fn broadcast_channel(&self, channel: &str, enrich_as: &str, payload: Value) {
        let enriched = self.enrich_payload(enrich_as, payload);
        let message = json!({
            "type": channel,
            "payload": enriched,
        });
        self.publish_message(&message);
        self.hub.broadcast(message).await;
    }

    pub fn broadcast_channel_now(&self, channel: &str, enrich_as: &str, payload: Value) {
        let enriched = self.enrich_payload(enrich_as, payload);
        let message = json!({
            "type": channel,
            "payload": enriched,
        });
        broadcast_now(&self.hub, message, self.event_bus.as_ref());
    }

    /// Enriched envelope → [`RuntimeEventBus`] only (no `/ws/events` fanout).
    ///
    /// Used for high-volume desktop-shell events (Twitch chat) that OBS must not receive.
    pub fn publish_event_bus_only(&self, channel: &str, enrich_as: &str, payload: Value) {
        let enriched = self.enrich_payload(enrich_as, payload);
        let message = json!({
            "type": channel,
            "payload": enriched,
        });
        self.publish_message(&message);
    }

    /// Overlay/subtitle payloads are already shaped; enrich in-place for stale guards.
    pub async fn broadcast_overlay_body(&self, channel: &str, enrich_as: &str, mut body: Value) {
        let enriched = self.enrich_payload(enrich_as, body.take());
        let message = json!({
            "type": channel,
            "payload": enriched,
        });
        self.publish_message(&message);
        self.hub.broadcast(message).await;
    }

    pub fn broadcast_overlay_body_now(&self, channel: &str, enrich_as: &str, mut body: Value) {
        let enriched = self.enrich_payload(enrich_as, body.take());
        broadcast_now(
            &self.hub,
            json!({
                "type": channel,
                "payload": enriched,
            }),
            self.event_bus.as_ref(),
        );
    }

    fn publish_message(&self, message: &Value) {
        if let Some(bus) = &self.event_bus {
            bus.publish(message.clone());
        }
    }

    fn enrich_payload(&self, enrich_as: &str, payload: Value) -> Value {
        self.sequencer.enrich(enrich_as, payload)
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
        // event_bus was already published in broadcast_now before this job was enqueued.
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

fn broadcast_now(events: &EventsHub, message: Value, event_bus: Option<&RuntimeEventBus>) {
    if let Some(bus) = event_bus {
        bus.publish(message.clone());
    }
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
    use std::sync::Arc;

    use super::*;
    use crate::event_bus::RuntimeEventBus;
    use crate::event_sequence::EventSequencer;

    #[test]
    fn enrich_as_can_differ_from_channel() {
        let publisher =
            WsEventPublisher::new(EventsHub::new(), Arc::new(EventSequencer::default()));
        let body = publisher
            .sequencer()
            .enrich("runtime_status", json!({ "running": true }));
        assert_eq!(body["event_type"], "runtime_status");
    }

    /// Verify that broadcast_now publishes to event_bus exactly once even when called
    /// outside a Tokio runtime (the sync-fallback path must not double-publish).
    #[test]
    fn broadcast_now_publishes_event_bus_exactly_once_outside_tokio() {
        let hub = EventsHub::new();
        let bus = RuntimeEventBus::new();
        let bus_for_thread = bus.clone();

        std::thread::spawn(move || {
            broadcast_now(&hub, json!({"type": "test_event"}), Some(&bus_for_thread));
        })
        .join()
        .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(
            bus.diagnostics().publish_count,
            1,
            "event_bus must be published exactly once"
        );
    }

    #[test]
    fn publish_event_bus_only_does_not_touch_ws_hub_last_message() {
        let hub = EventsHub::new();
        let bus = RuntimeEventBus::new();
        let publisher = WsEventPublisher::with_event_bus(
            hub.clone(),
            Arc::new(EventSequencer::default()),
            Some(bus.clone()),
        );
        publisher.publish_event_bus_only(
            "twitch_chat_message",
            "twitch_chat_message",
            json!({ "text": "hi" }),
        );
        assert_eq!(bus.diagnostics().publish_count, 1);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        let last = rt.block_on(hub.last_message("twitch_chat_message"));
        assert!(
            last.is_none(),
            "bus-only publish must not store OBS/WS last_message"
        );
    }
}
