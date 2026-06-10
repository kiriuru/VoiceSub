use std::sync::Arc;

use super::runtime::resolve_base_url;
use super::state::HttpState;
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;

pub async fn list_exports(State(state): State<Arc<HttpState>>) -> Response {
    match state.export_service.list_exports() {
        Ok(list) => Json(list).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "message": err.to_string() })),
        )
            .into_response(),
    }
}

pub async fn export_diagnostics(State(state): State<Arc<HttpState>>) -> Response {
    let runtime_status = state.orchestrator.status(state.as_ref()).await;
    let config_payload = {
        let store = state.config.read().await;
        store.payload().clone()
    };
    let base = resolve_base_url(state.as_ref()).await;
    match state.export_service.export_diagnostics_bundle(
        runtime_status,
        config_payload,
        &state.paths,
        &base,
    ) {
        Ok(path) => serve_zip_file(path),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "message": err.to_string() })),
        )
            .into_response(),
    }
}

fn serve_zip_file(path: std::path::PathBuf) -> Response {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("diagnostics.zip")
        .to_string();
    match std::fs::read(&path) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/zip")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            )
            .body(Body::from(bytes))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "message": err.to_string() })),
        )
            .into_response(),
    }
}
