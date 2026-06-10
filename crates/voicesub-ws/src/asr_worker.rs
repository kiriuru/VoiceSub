use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};

use futures_util::{SinkExt, StreamExt};

use serde_json::Value;

use tokio::sync::{mpsc, RwLock};

use tracing::{info, instrument, warn};

use voicesub_browser::BrowserAsrService;

use voicesub_types::{parse_worker_message_type, AsrWorkerHello, WsMessageType};

#[derive(Debug, Clone, Default)]

pub struct AsrWorkerSnapshot {
    pub worker_connected: bool,

    pub transport_id: u64,

    pub last_seen_at_ms: Option<u64>,

    pub recognition_state: String,
}

/// Browser worker WebSocket hub; delegates ingress to `BrowserAsrService`.

#[derive(Clone)]

pub struct AsrWorkerHub {
    service: Arc<BrowserAsrService>,

    snapshot: Arc<RwLock<AsrWorkerSnapshot>>,
}

impl AsrWorkerHub {
    pub fn new(service: Arc<BrowserAsrService>) -> Self {
        Self {
            service,

            snapshot: Arc::new(RwLock::new(AsrWorkerSnapshot::default())),
        }
    }

    pub fn service(&self) -> Arc<BrowserAsrService> {
        self.service.clone()
    }

    pub async fn snapshot(&self) -> AsrWorkerSnapshot {
        let diag = self.service.diagnostics().await;

        let cached = self.snapshot.read().await.clone();

        AsrWorkerSnapshot {
            worker_connected: diag.worker_connected,

            transport_id: cached.transport_id,

            last_seen_at_ms: diag.last_seen_at_ms,

            recognition_state: diag.recognition_state,
        }
    }

    pub async fn send_control(&self, action: &str, reason: Option<&str>) -> bool {
        self.service.send_control(action, reason).await
    }

    #[instrument(skip(self, socket))]

    pub async fn serve_connection(self, socket: WebSocket) {
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<String>(32);

        let transport_id = self.service.register_connection(cmd_tx).await;

        {
            let mut snap = self.snapshot.write().await;

            *snap = AsrWorkerSnapshot {
                worker_connected: true,

                transport_id,

                last_seen_at_ms: Some(now_ms()),

                recognition_state: "idle".into(),
            };
        }

        let (mut ws_tx, mut ws_rx) = socket.split();

        let hello = AsrWorkerHello::new(transport_id);

        let hello_json = serde_json::to_string(&hello).unwrap_or_default();

        if ws_tx.send(Message::Text(hello_json.into())).await.is_err() {
            self.service.disconnect(transport_id).await;

            return;
        }

        self.service.worker_connected().await;

        info!(transport_id, "asr worker connected");

        loop {
            tokio::select! {

                inbound = ws_rx.next() => {

                    match inbound {

                        Some(Ok(Message::Text(text))) => {

                            self.handle_text(transport_id, &text).await;

                        }

                        Some(Ok(Message::Close(_))) | None => break,

                        Some(Err(err)) => {

                            warn!(transport_id, error = %err, "asr worker read error");

                            break;

                        }

                        _ => {}

                    }

                }

                outbound = cmd_rx.recv() => {

                    match outbound {

                        Some(text) => {

                            if ws_tx.send(Message::Text(text.into())).await.is_err() {

                                break;

                            }

                        }

                        None => break,

                    }

                }

            }
        }

        self.service.disconnect(transport_id).await;

        {
            let mut snap = self.snapshot.write().await;

            if snap.transport_id == transport_id {
                snap.worker_connected = false;

                snap.recognition_state = "disconnected".into();
            }
        }

        info!(transport_id, "asr worker disconnected");
    }

    async fn handle_text(&self, transport_id: u64, text: &str) {
        let Ok(value) = serde_json::from_str::<Value>(text) else {
            return;
        };

        let Some(kind) = value.get("type").and_then(|v| v.as_str()) else {
            return;
        };

        match parse_worker_message_type(kind) {
            WsMessageType::ExternalAsrUpdate => {
                let _ = self
                    .service
                    .handle_external_update(transport_id, &value)
                    .await;
            }

            WsMessageType::BrowserAsrStatus | WsMessageType::BrowserAsrHeartbeat => {
                let _ = self.service.handle_status(transport_id, &value).await;
            }

            _ => {}
        }
    }
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {

    use super::*;

    use std::sync::atomic::{AtomicU64, Ordering};

    #[tokio::test]

    async fn snapshot_defaults_disconnected() {
        let hub = AsrWorkerHub::new(Arc::new(BrowserAsrService::new(Arc::new(|_| {}))));

        let snap = hub.snapshot().await;

        assert!(!snap.worker_connected);
    }

    #[tokio::test]

    async fn send_control_without_connection_returns_false() {
        let hub = AsrWorkerHub::new(Arc::new(BrowserAsrService::new(Arc::new(|_| {}))));

        assert!(!hub.send_control("stop", Some("test")).await);
    }

    #[tokio::test]

    async fn stale_generation_not_ingested() {
        let ingested = Arc::new(AtomicU64::new(0));

        let ingested_cb = ingested.clone();

        let service = Arc::new(BrowserAsrService::new(Arc::new(move |_| {
            ingested_cb.fetch_add(1, Ordering::Relaxed);
        })));

        let hub = AsrWorkerHub::new(service.clone());

        let (cmd_tx, _cmd_rx) = mpsc::channel(4);

        let transport_id = service.register_connection(cmd_tx).await;

        assert!(
            service
                .handle_status(
                    transport_id,
                    &serde_json::json!({

                        "type": "browser_asr_status",

                        "session_id": "session-a",

                        "generation_id": 3,

                        "recognition_state": "running",

                    })
                )
                .await
        );

        assert!(
            !service
                .handle_external_update(
                    transport_id,
                    &serde_json::json!({

                        "type": "external_asr_update",

                        "session_id": "session-a",

                        "generation_id": 2,

                        "partial": "old",

                        "is_final": false,

                    })
                )
                .await
        );

        assert_eq!(ingested.load(Ordering::Relaxed), 0);

        let diag = service.diagnostics().await;

        assert_eq!(diag.browser_stale_events_ignored, 1);

        let _ = hub;
    }
}
