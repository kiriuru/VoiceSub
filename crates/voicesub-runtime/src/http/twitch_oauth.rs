use std::sync::Arc;

use super::state::HttpState;
use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct TwitchOAuthCompleteRequest {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct TwitchOAuthOpenRequest {
    pub url: String,
}

fn normalize_token(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.to_ascii_lowercase().starts_with("oauth:") {
        trimmed.to_string()
    } else {
        format!("oauth:{trimmed}")
    }
}

pub async fn twitch_oauth_complete(
    State(state): State<Arc<HttpState>>,
    Json(body): Json<TwitchOAuthCompleteRequest>,
) -> Json<Value> {
    let token = normalize_token(&body.token);
    if token.is_empty() {
        return Json(json!({ "ok": false, "error": "empty token" }));
    }
    state.twitch_oauth.store(token);
    Json(json!({ "ok": true }))
}

pub async fn twitch_oauth_pending(State(state): State<Arc<HttpState>>) -> Json<Value> {
    match state.twitch_oauth.take() {
        Some(token) => Json(json!({ "ok": true, "token": token })),
        None => Json(json!({ "ok": false })),
    }
}

pub async fn twitch_oauth_open(Json(body): Json<TwitchOAuthOpenRequest>) -> Json<Value> {
    let trimmed = body.url.trim();
    if trimmed.is_empty() {
        return Json(json!({ "ok": false, "error": "empty url" }));
    }
    if !trimmed.starts_with("https://id.twitch.tv/") {
        return Json(json!({ "ok": false, "error": "only Twitch OAuth URLs are allowed" }));
    }
    info!(target: "voicesub.tts.oauth", url = %trimmed, "opening twitch oauth in system browser");
    match open::that(trimmed) {
        Ok(()) => Json(json!({ "ok": true })),
        Err(err) => Json(json!({ "ok": false, "error": err.to_string() })),
    }
}
