use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

const RECOMMENDED_OPENAI_TEXT_MODELS: &[&str] = &[
    "gpt-4o-mini",
    "gpt-4.1-mini",
    "gpt-4.1-nano",
    "gpt-4o",
    "gpt-4.1",
];

#[derive(Debug, Deserialize, Default)]
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

pub async fn list_models(Json(body): Json<OpenAiModelsRequest>) -> Response {
    let models: Vec<&str> = RECOMMENDED_OPENAI_TEXT_MODELS.to_vec();
    let _ = body.api_key;
    let _ = body.base_url;
    let _ = body.show_all;
    Json(json!({
        "models": models,
        "source": "static_recommended"
    }))
    .into_response()
}

pub async fn usable_models(Json(body): Json<OpenAiModelsRequest>) -> Response {
    list_models(Json(body)).await
}
