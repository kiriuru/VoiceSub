use axum::Json;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Browser Speech uses Chrome `getUserMedia`; core returns an empty device list for API parity.
pub async fn audio_inputs() -> Response {
    Json(json!({
        "devices": [],
        "source": "browser_getusermedia"
    }))
    .into_response()
}
