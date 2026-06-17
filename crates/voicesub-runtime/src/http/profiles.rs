use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use voicesub_config::ProfileStore;

use super::state::HttpState;

#[derive(Debug, Deserialize)]
pub struct ProfileWriteBody {
    #[serde(default)]
    pub payload: Value,
}

pub async fn list_profiles(State(state): State<Arc<HttpState>>) -> Response {
    let store = ProfileStore::new(state.paths.profiles_dir());
    let _ = store.ensure_default_profile();
    match store.list_profiles() {
        Ok(profiles) => Json(json!({ "profiles": profiles })).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "message": err.to_string() })),
        )
            .into_response(),
    }
}

pub async fn load_profile(
    State(state): State<Arc<HttpState>>,
    Path(name): Path<String>,
) -> Response {
    let store = ProfileStore::new(state.paths.profiles_dir());
    match store.load_profile(&name) {
        Ok(payload) => Json(json!({ "name": name, "payload": payload })).into_response(),
        Err(voicesub_config::ProfileError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "message": format!("Profile '{name}' does not exist.") })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "message": err.to_string() })),
        )
            .into_response(),
    }
}

pub async fn save_profile(
    State(state): State<Arc<HttpState>>,
    Path(name): Path<String>,
    Json(body): Json<ProfileWriteBody>,
) -> Response {
    let store = ProfileStore::new(state.paths.profiles_dir());
    match store.save_profile(&name, &body.payload) {
        Ok((path, payload)) => Json(json!({
            "name": name,
            "saved_to": path.display().to_string(),
            "payload": payload
        }))
        .into_response(),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "message": err.to_string() })),
        )
            .into_response(),
    }
}

pub async fn delete_profile(
    State(state): State<Arc<HttpState>>,
    Path(name): Path<String>,
) -> Response {
    let store = ProfileStore::new(state.paths.profiles_dir());
    match store.delete_profile(&name) {
        Ok(deleted) => Json(json!({ "name": name, "deleted": deleted })).into_response(),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "message": err.to_string() })),
        )
            .into_response(),
    }
}
