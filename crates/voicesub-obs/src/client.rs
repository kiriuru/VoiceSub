use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use thiserror::Error;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::warn;

use crate::auth::build_auth_response;

#[derive(Debug, Error)]
pub enum ObsClientError {
    #[error("OBS websocket requires a password, but none is configured.")]
    PasswordRequired,
    #[error("obs websocket auth failed")]
    AuthFailed,
    #[error("obs websocket request failed: {comment}")]
    RequestFailed { comment: String, code: Option<i64> },
    #[error("obs websocket protocol error: {0}")]
    Protocol(String),
    #[error("obs websocket io error: {0}")]
    Io(#[from] tokio_tungstenite::tungstenite::Error),
}

pub struct ObsWsClient {
    socket: WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    obs_studio_version: Option<String>,
    obs_websocket_version: Option<String>,
}

async fn send_message(
    socket: &mut WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    payload: String,
    seconds: u64,
) -> Result<(), ObsClientError> {
    timeout(
        Duration::from_secs(seconds),
        socket.send(Message::Text(payload.into())),
    )
    .await
    .map_err(|_| ObsClientError::Protocol("send timeout".into()))??;
    Ok(())
}

async fn recv_message(
    socket: &mut WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    seconds: u64,
) -> Result<String, ObsClientError> {
    loop {
        let message = timeout(Duration::from_secs(seconds), socket.next())
            .await
            .map_err(|_| ObsClientError::Protocol("recv timeout".into()))?
            .ok_or_else(|| ObsClientError::Protocol("socket closed".into()))??;
        match message {
            Message::Text(text) => return Ok(text.to_string()),
            Message::Ping(payload) => {
                let _ = socket.send(Message::Pong(payload)).await;
            }
            other => {
                warn!(?other, "unexpected obs websocket frame");
            }
        }
    }
}

impl ObsWsClient {
    pub async fn connect(host: &str, port: u16, password: &str) -> Result<Self, ObsClientError> {
        let url = format!("ws://{host}:{port}");
        let mut request = url
            .into_client_request()
            .map_err(|err| ObsClientError::Protocol(err.to_string()))?;
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            "obswebsocket.json".parse().unwrap(),
        );

        let (mut socket, _) = timeout(Duration::from_secs(3), connect_async(request))
            .await
            .map_err(|_| ObsClientError::Protocol("connect timeout".into()))??;

        let hello_raw = recv_message(&mut socket, 3).await?;
        let hello: Value = serde_json::from_str(&hello_raw)
            .map_err(|err| ObsClientError::Protocol(err.to_string()))?;
        if hello.get("op").and_then(|value| value.as_i64()) != Some(0) {
            return Err(ObsClientError::Protocol("missing hello".into()));
        }
        let hello_data = hello.get("d").cloned().unwrap_or(Value::Null);
        let mut identify = json!({
            "rpcVersion": 1,
            "eventSubscriptions": 0
        });
        if let Some(auth) = hello_data.get("authentication") {
            if password.is_empty() {
                return Err(ObsClientError::PasswordRequired);
            }
            let salt = auth
                .get("salt")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let challenge = auth
                .get("challenge")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            identify["authentication"] = json!(build_auth_response(password, salt, challenge));
        }

        send_message(
            &mut socket,
            json!({ "op": 1, "d": identify }).to_string(),
            3,
        )
        .await?;

        let identified_raw = recv_message(&mut socket, 3).await?;
        let identified: Value = serde_json::from_str(&identified_raw)
            .map_err(|err| ObsClientError::Protocol(err.to_string()))?;
        let op = identified
            .get("op")
            .and_then(|value| value.as_i64())
            .unwrap_or(-1);
        if op == 5 {
            return Err(ObsClientError::AuthFailed);
        }
        if op != 2 {
            return Err(ObsClientError::Protocol("identify failed".into()));
        }

        Ok(Self {
            socket,
            obs_studio_version: hello_data
                .get("obsStudioVersion")
                .and_then(|value| value.as_str())
                .map(str::to_string),
            obs_websocket_version: hello_data
                .get("obsWebSocketVersion")
                .and_then(|value| value.as_str())
                .map(str::to_string),
        })
    }

    pub fn versions(&self) -> (Option<&str>, Option<&str>) {
        (
            self.obs_studio_version.as_deref(),
            self.obs_websocket_version.as_deref(),
        )
    }

    pub async fn send_request(
        &mut self,
        request_type: &str,
        request_data: Value,
    ) -> Result<Value, ObsClientError> {
        let request_id = uuid::Uuid::new_v4().to_string();
        send_message(
            &mut self.socket,
            json!({
                "op": 6,
                "d": {
                    "requestType": request_type,
                    "requestId": request_id,
                    "requestData": request_data
                }
            })
            .to_string(),
            2,
        )
        .await?;

        loop {
            let raw = recv_message(&mut self.socket, 3).await?;
            let message: Value = serde_json::from_str(&raw)
                .map_err(|err| ObsClientError::Protocol(err.to_string()))?;
            if message.get("op").and_then(|value| value.as_i64()) != Some(7) {
                continue;
            }
            let data = message.get("d").cloned().unwrap_or(Value::Null);
            if data.get("requestId").and_then(|value| value.as_str()) != Some(request_id.as_str()) {
                continue;
            }
            let status = data.get("requestStatus").cloned().unwrap_or(Value::Null);
            if status.get("result").and_then(|value| value.as_bool()) != Some(true) {
                let comment = status
                    .get("comment")
                    .and_then(|value| value.as_str())
                    .unwrap_or("request failed");
                let code = status.get("code").and_then(|value| value.as_i64());
                return Err(ObsClientError::RequestFailed {
                    comment: comment.to_string(),
                    code,
                });
            }
            return Ok(data.get("responseData").cloned().unwrap_or(Value::Null));
        }
    }

    pub async fn ping(&mut self) -> Result<(), ObsClientError> {
        timeout(Duration::from_secs(5), async {
            self.socket.send(Message::Ping(Vec::new().into())).await?;
            loop {
                let message = self
                    .socket
                    .next()
                    .await
                    .ok_or_else(|| ObsClientError::Protocol("socket closed".into()))??;
                match message {
                    Message::Pong(_) => return Ok(()),
                    Message::Ping(payload) => {
                        self.socket.send(Message::Pong(payload)).await?;
                    }
                    Message::Text(_) => continue,
                    Message::Close(_) => {
                        return Err(ObsClientError::Protocol("socket closed".into()));
                    }
                    Message::Binary(_) | Message::Frame(_) => continue,
                }
            }
        })
        .await
        .map_err(|_| ObsClientError::Protocol("ping pong timeout".into()))?
    }

    pub async fn close(mut self) {
        let _ = self.socket.close(None).await;
    }

    #[cfg(test)]
    pub(crate) fn from_socket_for_test(
        socket: WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    ) -> Self {
        Self {
            socket,
            obs_studio_version: None,
            obs_websocket_version: None,
        }
    }
}

/// Unified OBS client handle for live websocket and test mocks.
pub(crate) enum ObsClientHandle {
    WebSocket(Box<ObsWsClient>),
    #[cfg(test)]
    Mock(MockObsClient),
}

impl ObsClientHandle {
    pub async fn send_request(
        &mut self,
        request_type: &str,
        request_data: Value,
    ) -> Result<Value, ObsClientError> {
        match self {
            Self::WebSocket(client) => client.send_request(request_type, request_data).await,
            #[cfg(test)]
            Self::Mock(client) => client.send_request(request_type, request_data).await,
        }
    }

    pub async fn ping(&mut self) -> Result<(), ObsClientError> {
        match self {
            Self::WebSocket(client) => client.ping().await,
            #[cfg(test)]
            Self::Mock(client) => client.ping().await,
        }
    }

    pub async fn close(self) {
        match self {
            Self::WebSocket(client) => client.close().await,
            #[cfg(test)]
            Self::Mock(client) => client.close().await,
        }
    }
}

#[cfg(test)]
#[derive(Clone, Default)]
pub struct MockObsClient {
    pub requests: std::sync::Arc<std::sync::Mutex<Vec<(String, Value)>>>,
    pub fail_caption: bool,
    pub fail_caption_inactive: bool,
    pub fail_ping: bool,
}

#[cfg(test)]
impl MockObsClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn recorded_requests(&self) -> Vec<(String, Value)> {
        self.requests.lock().unwrap().clone()
    }

    pub async fn send_request(
        &mut self,
        request_type: &str,
        request_data: Value,
    ) -> Result<Value, ObsClientError> {
        self.requests
            .lock()
            .unwrap()
            .push((request_type.to_string(), request_data));
        if request_type == "SendStreamCaption" && self.fail_caption {
            return Err(ObsClientError::RequestFailed {
                comment: "mock caption failure".into(),
                code: None,
            });
        }
        if request_type == "SendStreamCaption" && self.fail_caption_inactive {
            return Err(ObsClientError::RequestFailed {
                comment: "mock stream inactive".into(),
                code: Some(501),
            });
        }
        if request_type == "GetStreamStatus" {
            return Ok(json!({
                "outputActive": true,
                "outputReconnecting": false
            }));
        }
        Ok(Value::Null)
    }

    pub async fn ping(&mut self) -> Result<(), ObsClientError> {
        if self.fail_ping {
            Err(ObsClientError::Protocol("mock ping failure".into()))
        } else {
            Ok(())
        }
    }

    pub async fn close(self) {}
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    use super::*;

    async fn spawn_pong_server() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            while let Some(Ok(message)) = socket.next().await {
                if matches!(message, Message::Ping(_)) {
                    let _ = socket.send(Message::Pong(Vec::new().into())).await;
                    break;
                }
            }
        });
        addr
    }

    #[tokio::test]
    async fn ping_waits_for_pong() {
        let addr = spawn_pong_server().await;
        let (socket, _) = connect_async(format!("ws://{addr}"))
            .await
            .expect("test websocket connect");
        let mut client = ObsWsClient::from_socket_for_test(socket);
        client.ping().await.expect("ping should receive pong");
        client.close().await;
    }

    #[tokio::test]
    async fn ping_times_out_without_pong() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _socket = accept_async(stream).await.unwrap();
            tokio::time::sleep(Duration::from_secs(6)).await;
        });
        let (socket, _) = connect_async(format!("ws://{addr}"))
            .await
            .expect("test websocket connect");
        let mut client = ObsWsClient::from_socket_for_test(socket);
        let err = client.ping().await.expect_err("ping should time out");
        assert!(matches!(err, ObsClientError::Protocol(message) if message.contains("pong")));
    }
}
