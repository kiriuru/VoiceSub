use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::{Mutex, Notify, RwLock};
use tracing::{info, instrument, warn};
use voicesub_types::WsMessage;

use crate::trace::WsLog;

type SocketId = u64;

pub const DEFAULT_OUTBOUND_QUEUE_MAX: usize = 128;

#[derive(Debug, Default)]
pub struct EventsHubDiagnostics {
    pub connections_active: usize,
    pub broadcast_count: u64,
    pub send_failures: u64,
    pub dead_connections_removed: u64,
    pub dropped_oldest: u64,
    pub queue_max_depth_observed: u64,
}

struct ClientState {
    queue: Arc<Mutex<VecDeque<Message>>>,
    notify: Arc<Notify>,
    queue_max: usize,
}

/// Port of SST `WebSocketManager` — broadcast + replay + bounded per-socket queue.
#[derive(Clone)]
pub struct EventsHub {
    inner: Arc<EventsHubInner>,
}

struct EventsHubInner {
    next_id: AtomicU64,
    clients: RwLock<HashMap<SocketId, ClientState>>,
    last_by_type: RwLock<HashMap<String, Value>>,
    broadcast_count: AtomicU64,
    send_failures: AtomicU64,
    dead_connections_removed: AtomicU64,
    dropped_oldest: AtomicU64,
    queue_max_depth_observed: AtomicU64,
    outbound_queue_max: usize,
    log: WsLog,
}

impl Default for EventsHub {
    fn default() -> Self {
        Self::new()
    }
}

impl EventsHub {
    pub fn new() -> Self {
        Self::with_outbound_queue_max(DEFAULT_OUTBOUND_QUEUE_MAX)
    }

    pub fn with_log(log: WsLog) -> Self {
        Self::with_options(DEFAULT_OUTBOUND_QUEUE_MAX, log)
    }

    pub fn with_outbound_queue_max(outbound_queue_max: usize) -> Self {
        Self::with_options(outbound_queue_max, WsLog::default())
    }

    pub fn with_options(outbound_queue_max: usize, log: WsLog) -> Self {
        Self {
            inner: Arc::new(EventsHubInner {
                next_id: AtomicU64::new(1),
                clients: RwLock::new(HashMap::new()),
                last_by_type: RwLock::new(HashMap::new()),
                broadcast_count: AtomicU64::new(0),
                send_failures: AtomicU64::new(0),
                dead_connections_removed: AtomicU64::new(0),
                dropped_oldest: AtomicU64::new(0),
                queue_max_depth_observed: AtomicU64::new(0),
                outbound_queue_max: outbound_queue_max.max(1),
                log,
            }),
        }
    }

    pub fn diagnostics(&self) -> EventsHubDiagnostics {
        let active = self.inner.clients.try_read().map(|c| c.len()).unwrap_or(0);
        EventsHubDiagnostics {
            connections_active: active,
            broadcast_count: self.inner.broadcast_count.load(Ordering::Relaxed),
            send_failures: self.inner.send_failures.load(Ordering::Relaxed),
            dead_connections_removed: self
                .inner
                .dead_connections_removed
                .load(Ordering::Relaxed),
            dropped_oldest: self.inner.dropped_oldest.load(Ordering::Relaxed),
            queue_max_depth_observed: self
                .inner
                .queue_max_depth_observed
                .load(Ordering::Relaxed),
        }
    }

    #[instrument(skip(self, socket))]
    pub async fn serve_connection(self, socket: WebSocket) {
        let socket_id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (mut ws_tx, mut ws_rx) = socket.split();
        let queue: Arc<Mutex<VecDeque<Message>>> = Arc::new(Mutex::new(VecDeque::new()));
        let notify = Arc::new(Notify::new());
        let queue_max = self.inner.outbound_queue_max;

        {
            let mut clients = self.inner.clients.write().await;
            clients.insert(
                socket_id,
                ClientState {
                    queue: queue.clone(),
                    notify: notify.clone(),
                    queue_max,
                },
            );
            let active = clients.len();
            self.inner.log.connection_open(socket_id, active, queue_max);
            info!(socket_id, connections_active = active, "ws events client connected");
        }

        let hub = self.clone();
        let send_task = tokio::spawn(async move {
            loop {
                let next = {
                    let mut guard = queue.lock().await;
                    guard.pop_front()
                };
                if let Some(msg) = next {
                    if ws_tx.send(msg).await.is_err() {
                        break;
                    }
                    continue;
                }
                notify.notified().await;
            }
        });

        let _ = self.send_direct(socket_id, WsMessage::hello_events()).await;
        let _ = self
            .replay_last(
                socket_id,
                &[
                    "runtime_update",
                    "subtitle_payload_update",
                    "overlay_update",
                ],
            )
            .await;

        while let Some(Ok(Message::Text(_))) = ws_rx.next().await {
            // Dashboard/overlay clients are receive-only on this endpoint in SST.
        }

        self.disconnect(socket_id).await;
        send_task.abort();
        let _ = hub;
    }

    pub async fn broadcast(&self, message: Value) {
        let message_type = message
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !message_type.is_empty() {
            let mut last = self.inner.last_by_type.write().await;
            last.insert(message_type.clone(), message.clone());
        }

        self.inner
            .broadcast_count
            .fetch_add(1, Ordering::Relaxed);

        let clients = self.inner.clients.read().await;
        let connection_count = clients.len();
        if !message_type.is_empty() {
            self.inner
                .log
                .broadcast(&message_type, connection_count);
        }

        let payload = match serde_json::to_string(&message) {
            Ok(s) => s,
            Err(err) => {
                warn!(error = %err, "events broadcast serialize failed");
                return;
            }
        };
        let text_msg = Message::Text(payload.into());
        for client in clients.values() {
            self.enqueue_to_client(client, text_msg.clone());
        }
    }

    pub async fn send_direct(&self, socket_id: SocketId, message: WsMessage) -> bool {
        let payload = match serde_json::to_string(&message) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let clients = self.inner.clients.read().await;
        let Some(client) = clients.get(&socket_id) else {
            return false;
        };
        self.enqueue_to_client(client, Message::Text(payload.into()));
        true
    }

    pub async fn replay_last(&self, socket_id: SocketId, message_types: &[&str]) -> bool {
        let last = self.inner.last_by_type.read().await;
        let mut ok = true;
        for kind in message_types {
            if let Some(msg) = last.get(*kind) {
                let payload = serde_json::to_string(msg).unwrap_or_default();
                let clients = self.inner.clients.read().await;
                if let Some(client) = clients.get(&socket_id) {
                    self.enqueue_to_client(client, Message::Text(payload.into()));
                } else {
                    ok = false;
                }
            }
        }
        ok
    }

    fn enqueue_to_client(&self, client: &ClientState, message: Message) {
        let message_type = match &message {
            Message::Text(text) => serde_json::from_str::<Value>(text.as_ref())
                .ok()
                .and_then(|value| {
                    value
                        .get("type")
                        .and_then(|kind| kind.as_str())
                        .map(str::to_string)
                })
                .unwrap_or_default(),
            _ => String::new(),
        };
        let dropped = {
            let mut guard = match client.queue.try_lock() {
                Ok(guard) => guard,
                Err(_) => {
                    self.inner.send_failures.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            };
            if guard.len() >= client.queue_max {
                guard.pop_front();
                self.inner.dropped_oldest.fetch_add(1, Ordering::Relaxed);
                self.inner
                    .log
                    .outbound_queue_drop_oldest(&message_type);
            }
            guard.push_back(message);
            let depth = guard.len();
            if depth >= client.queue_max.saturating_sub(4).max(1) {
                self.inner.log.outbound_queue_pressure(
                    depth,
                    client.queue_max,
                    &message_type,
                );
            }
            let depth = depth as u64;
            let mut observed = self
                .inner
                .queue_max_depth_observed
                .load(Ordering::Relaxed);
            if depth > observed {
                self.inner
                    .queue_max_depth_observed
                    .store(depth, Ordering::Relaxed);
                observed = depth;
            }
            observed
        };
        let _ = dropped;
        client.notify.notify_one();
    }

    pub async fn last_message(&self, message_type: &str) -> Option<Value> {
        self.inner
            .last_by_type
            .read()
            .await
            .get(message_type)
            .cloned()
    }

    async fn disconnect(&self, socket_id: SocketId) {
        let mut clients = self.inner.clients.write().await;
        if clients.remove(&socket_id).is_some() {
            self.inner
                .dead_connections_removed
                .fetch_add(1, Ordering::Relaxed);
            let active = clients.len();
            self.inner.log.connection_closed(socket_id, active);
            info!(socket_id, connections_active = active, "ws events client disconnected");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_start_empty() {
        let hub = EventsHub::new();
        let diag = hub.diagnostics();
        assert_eq!(diag.connections_active, 0);
        assert_eq!(diag.send_failures, 0);
        assert_eq!(diag.dropped_oldest, 0);
    }
}
