use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use voicesub_logging::ui_trace;

use super::state::HttpState;

#[derive(Debug, Deserialize)]
pub struct ClientLogEventRequest {
    pub channel: String,
    pub message: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub details: Option<BTreeMap<String, Value>>,
}

pub async fn logs_client_event(
    State(state): State<Arc<HttpState>>,
    Json(body): Json<ClientLogEventRequest>,
) -> Json<voicesub_logging::ClientLogResult> {
    let result = state.session_log.log(
        &body.channel,
        &body.message,
        body.source.as_deref(),
        body.details,
    );
    Json(result)
}

#[derive(Debug, Deserialize)]
pub struct UiTraceRequest {
    pub surface: String,
    pub phase: String,
    pub event: String,
    #[serde(default)]
    pub fields: Option<Value>,
}

pub async fn logs_ui_trace(Json(body): Json<UiTraceRequest>) -> Json<Value> {
    let fields = body.fields.unwrap_or_else(|| json!({}));
    ui_trace(&body.surface, &body.phase, &body.event, fields);
    Json(json!({ "logged": true }))
}
