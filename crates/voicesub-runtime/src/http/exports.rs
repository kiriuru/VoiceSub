use std::sync::Arc;

use voicesub_config::read_full_logging_enabled;

use super::runtime::resolve_base_url;
use super::state::HttpState;
use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

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
    let include_deep_traces = read_full_logging_enabled(&config_payload);
    match state.export_service.export_diagnostics_bundle(
        runtime_status,
        config_payload,
        &state.paths,
        &base,
        include_deep_traces,
    ) {
        Ok(path) => serve_zip_file(path).await,
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "message": err.to_string() })),
        )
            .into_response(),
    }
}

async fn serve_zip_file(path: std::path::PathBuf) -> Response {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("diagnostics.zip")
        .to_string();
    match tokio::fs::read(&path).await {
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
