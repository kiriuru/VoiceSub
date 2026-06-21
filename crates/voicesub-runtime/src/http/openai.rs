use axum::Json;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

const RECOMMENDED_OPENAI_TEXT_MODELS: &[&str] = &[
    "gpt-4o-mini",
    "gpt-4.1-mini",
    "gpt-4.1-nano",
    "gpt-4o",
    "gpt-4.1",
];

/// Accepted request body for OpenAI model endpoints.
/// Fields are part of the API contract (clients may send them); the current
/// implementation returns a static recommended list and does not call OpenAI.
#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct OpenAiModelsRequest {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub show_all: bool,
}

pub async fn recommended_models() -> Response {
    Json(json!({
        "models": RECOMMENDED_OPENAI_TEXT_MODELS,
        "recommended": true
    }))
    .into_response()
}

pub async fn list_models(Json(_body): Json<OpenAiModelsRequest>) -> Response {
    Json(json!({
        "models": RECOMMENDED_OPENAI_TEXT_MODELS,
        "source": "static_recommended"
    }))
    .into_response()
}

/// Alias for `list_models`; retained for API surface compatibility.
pub async fn usable_models(body: Json<OpenAiModelsRequest>) -> Response {
    list_models(body).await
}
