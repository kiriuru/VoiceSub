use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::{Value, json};

use super::state::HttpState;

#[derive(Debug, Deserialize)]
pub struct UiSyncRequest {
    #[serde(default)]
    pub ui: Value,
}

pub async fn ui_sync(
    State(state): State<Arc<HttpState>>,
    Json(body): Json<UiSyncRequest>,
) -> impl IntoResponse {
    let ui = if body.ui.is_object() {
        body.ui
    } else {
        json!({})
    };
    state
        .events
        .broadcast(json!({
            "type": "ui_config_sync",
            "payload": { "ui": ui }
        }))
        .await;
    Json(json!({ "ok": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_sync_request_defaults_empty_ui_object() {
        let req: UiSyncRequest = serde_json::from_str("{}").expect("deserialize");
        assert!(req.ui.is_null());
    }

    #[test]
    fn ui_sync_request_accepts_ui_section() {
        let req: UiSyncRequest = serde_json::from_str(
            "{\"ui\":{\"theme\":\"dark\",\"palette\":{\"accent\":\"#ff0000\"}}}",
        )
        .expect("deserialize");
        assert_eq!(req.ui.get("theme").and_then(|v| v.as_str()), Some("dark"));
    }
}
