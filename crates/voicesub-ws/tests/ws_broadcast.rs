use std::time::Duration;

use axum::{Router, extract::WebSocketUpgrade, response::IntoResponse, routing::get};
use futures_util::StreamExt;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use voicesub_ws::EventsHub;

struct TestServer {
    addr: std::net::SocketAddr,
    shutdown: tokio::task::JoinHandle<()>,
}

impl TestServer {
    async fn drop(self) {
        self.shutdown.abort();
        let _ = self.shutdown.await;
    }
}

async fn spawn_events_server(hub: EventsHub) -> TestServer {
    let app = Router::new().route(
        "/ws/events",
        get({
            let hub = hub.clone();
            move |ws: WebSocketUpgrade| {
                let hub = hub.clone();
                async move {
                    ws.on_upgrade(move |socket| async move {
                        hub.serve_connection(socket).await;
                    })
                    .into_response()
                }
            }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let shutdown = tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, app).await {
            tracing::warn!(error = %err, "test ws server exited");
        }
    });
    TestServer { addr, shutdown }
}

#[tokio::test]
async fn events_hub_broadcasts_to_connected_client() {
    let hub = EventsHub::new();
    let server = spawn_events_server(hub.clone()).await;
    let addr = server.addr;

    let (mut socket, _) = connect_async(format!("ws://{addr}/ws/events"))
        .await
        .expect("connect");
    let hello = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .expect("hello timeout")
        .expect("frame")
        .expect("ok");
    let hello_text = match hello {
        Message::Text(text) => text,
        other => panic!("unexpected hello frame: {other:?}"),
    };
    let hello_json: serde_json::Value = serde_json::from_str(&hello_text).expect("hello json");
    assert_eq!(hello_json["type"], "hello");

    hub.broadcast(json!({
        "type": "transcript_update",
        "payload": { "text": "ping", "is_final": false }
    }))
    .await;

    let update = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .expect("update timeout")
        .expect("frame")
        .expect("ok");
    let update_text = match update {
        Message::Text(text) => text,
        other => panic!("unexpected update frame: {other:?}"),
    };
    let update_json: serde_json::Value = serde_json::from_str(&update_text).expect("update json");
    assert_eq!(update_json["type"], "transcript_update");
    assert_eq!(update_json["payload"]["text"], "ping");

    let _ = socket.close(None).await;
    server.drop().await;
}
