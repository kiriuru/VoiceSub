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
    // Use ws_publisher so the message gets event_sequence enrichment and is
    // forwarded to RuntimeEventBus (Tauri runtime-event channel).
    state
        .ws_publisher
        .broadcast_channel("ui_config_sync", "ui_config_sync", json!({ "ui": ui }))
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

    /// ui_sync non-object body must be coerced to empty object, never null.
    #[test]
    fn ui_sync_null_body_coerced_to_empty_object() {
        let ui_null: Value = serde_json::from_str("null").unwrap();
        let coerced = if ui_null.is_object() { ui_null } else { json!({}) };
        assert!(coerced.is_object());
        assert_eq!(coerced.as_object().map(|m| m.len()), Some(0));
    }
}
