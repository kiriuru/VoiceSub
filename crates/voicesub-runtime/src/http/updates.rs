use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use voicesub_types::PROJECT_VERSION;

/// GitHub release polling is deferred (roadmap Q-G1); returns current local version only.
pub async fn check_updates() -> Response {
    Json(json!({
        "version": PROJECT_VERSION,
        "product": "VoiceSub",
        "update_available": false,
        "latest_known_version": PROJECT_VERSION,
        "message": "Update check is deferred for VoiceSub 0.5.0."
    }))
    .into_response()
}
