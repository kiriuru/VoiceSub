use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::json;
use tokio::sync::mpsc;
use voicesub_browser::BrowserAsrService;

#[tokio::test]
async fn stale_generation_is_ignored() {
    let ingested = Arc::new(AtomicU64::new(0));
    let ingested_cb = ingested.clone();
    let service = BrowserAsrService::new(Arc::new(move |_| {
        ingested_cb.fetch_add(1, Ordering::Relaxed);
    }));

    let (cmd_tx, _cmd_rx) = mpsc::channel(4);
    let transport_id = service.register_connection(cmd_tx).await;

    assert!(
        service
            .handle_status(
                transport_id,
                &json!({
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
                &json!({
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
    assert_eq!(service.diagnostics().await.browser_stale_events_ignored, 1);
}

#[tokio::test]
async fn external_update_forwards_segment_and_forced_final() {
    let captured = Arc::new(Mutex::new(None));
    let captured_cb = captured.clone();
    let service = BrowserAsrService::new(Arc::new(move |update| {
        *captured_cb.lock().unwrap() = Some(update);
    }));

    let (cmd_tx, _cmd_rx) = mpsc::channel(4);
    let transport_id = service.register_connection(cmd_tx).await;

    assert!(
        service
            .handle_external_update(
                transport_id,
                &json!({
                    "type": "external_asr_update",
                    "session_id": "session-b",
                    "generation_id": 4,
                    "client_segment_id": "browser-seg-4",
                    "partial": "",
                    "final": "hello world",
                    "is_final": true,
                    "forced_final": true,
                })
            )
            .await
    );

    let update = captured.lock().unwrap().clone().expect("ingested");
    assert_eq!(update.client_segment_id.as_deref(), Some("browser-seg-4"));
    assert!(update.forced_final);
    assert_eq!(update.final_text, "hello world");
}

#[tokio::test]
async fn worker_disconnect_invokes_lifecycle_hook() {
    let disconnected = Arc::new(AtomicU64::new(0));
    let disconnected_cb = disconnected.clone();
    let service = BrowserAsrService::with_hooks(
        Arc::new(|_| {}),
        None,
        Some(Arc::new(move || {
            disconnected_cb.fetch_add(1, Ordering::Relaxed);
        })),
        None,
    );

    let (cmd_tx, _cmd_rx) = mpsc::channel(4);
    let transport_id = service.register_connection(cmd_tx).await;
    service.disconnect(transport_id).await;

    assert_eq!(disconnected.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn send_control_delivers_browser_asr_control_frame() {
    let service = BrowserAsrService::new(Arc::new(|_| {}));
    let (cmd_tx, mut cmd_rx) = mpsc::channel(4);
    let transport_id = service.register_connection(cmd_tx).await;

    assert!(service.send_control("stop", Some("runtime_stop")).await);
    let frame = cmd_rx.recv().await.expect("control frame");
    let value: serde_json::Value = serde_json::from_str(&frame).expect("json");
    assert_eq!(value["type"], "browser_asr_control");
    assert_eq!(value["action"], "stop");
    assert_eq!(value["reason"], "runtime_stop");
    assert_eq!(value["transport_id"], transport_id);
}

#[tokio::test]
async fn register_connection_drops_previous_outbound_without_stop() {
    let service = BrowserAsrService::new(Arc::new(|_| {}));
    let (first_tx, mut first_rx) = mpsc::channel(4);
    let first_id = service.register_connection(first_tx).await;

    let (second_tx, mut second_rx) = mpsc::channel(4);
    let second_id = service.register_connection(second_tx).await;
    assert_ne!(first_id, second_id);

    // Previous channel is closed (dropped) so the old write loop can exit; no stop frame.
    assert!(first_rx.recv().await.is_none());

    assert!(service.send_control("stop", Some("runtime_stop")).await);
    let frame = second_rx.recv().await.expect("control on new transport");
    let value: serde_json::Value = serde_json::from_str(&frame).expect("json");
    assert_eq!(value["transport_id"], second_id);
}
