use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::warn;

/// Curated chat models for subtitle translation (official OpenAI catalog, 2026).
/// Prefer cost-efficient multilingual chat IDs; keep aliases after concrete IDs.
/// Source: https://platform.openai.com/docs/models and /docs/models/all
const RECOMMENDED_OPENAI_CHAT_MODELS: &[&str] = &[
    "gpt-5.6-luna",
    "gpt-5.6-terra",
    "gpt-5.6-sol",
    "gpt-5.6",
    "gpt-5.4-nano",
    "gpt-5.4-mini",
    "gpt-5.4",
    "gpt-5-nano",
    "gpt-5-mini",
    "gpt-5",
    "gpt-4.1-nano",
    "gpt-4.1-mini",
    "gpt-4.1",
    "gpt-4o-mini",
    "gpt-4o",
];

/// Accepted request body for OpenAI-compatible model endpoints.
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
        "models": RECOMMENDED_OPENAI_CHAT_MODELS,
        "recommended": true,
        "source": "official_catalog_2026"
    }))
    .into_response()
}

pub async fn list_models(Json(body): Json<OpenAiModelsRequest>) -> Response {
    match fetch_compatible_models(&body).await {
        Ok(payload) => Json(payload).into_response(),
        Err((status, message)) => (
            status,
            Json(json!({
                "ok": false,
                "error": message,
                "models": [],
            })),
        )
            .into_response(),
    }
}

/// Alias for `list_models`; retained for API surface compatibility.
pub async fn usable_models(body: Json<OpenAiModelsRequest>) -> Response {
    list_models(body).await
}

async fn fetch_compatible_models(body: &OpenAiModelsRequest) -> Result<Value, (StatusCode, String)> {
    let base_url = normalize_base_url(body.base_url.as_deref());
    let official_openai = is_official_openai_host(&base_url);
    let api_key = body.api_key.trim();
    if official_openai && api_key.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "OpenAI API key is required to list models.".into(),
        ));
    }

    let models_url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .connect_timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to build HTTP client: {err}"),
            )
        })?;

    let mut request = client.get(&models_url);
    if !api_key.is_empty() {
        request = request.bearer_auth(api_key);
    }

    let response = request.send().await.map_err(|err| {
        warn!(error = %err, url = %models_url, "openai-compatible models request failed");
        (
            StatusCode::BAD_GATEWAY,
            format!("Failed to reach models endpoint: {err}"),
        )
    })?;

    let status = response.status();
    let body_text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        let detail = truncate(&body_text, 240);
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("Models endpoint HTTP {status}: {detail}"),
        ));
    }

    let payload: Value = serde_json::from_str(&body_text).map_err(|err| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Models endpoint returned invalid JSON: {err}"),
        )
    })?;

    let mut ids = extract_model_ids(&payload);
    if official_openai {
        ids.retain(|id| is_chat_completion_model(id));
    }
    ids.sort();
    ids.dedup();

    let recommended = recommended_present_in(&ids);
    let models = if body.show_all || !official_openai {
        ids
    } else if recommended.is_empty() {
        RECOMMENDED_OPENAI_CHAT_MODELS
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    } else {
        recommended.clone()
    };

    let recommended_models = if recommended.is_empty() {
        RECOMMENDED_OPENAI_CHAT_MODELS
            .iter()
            .map(|id| (*id).to_string())
            .collect::<Vec<_>>()
    } else {
        recommended
    };

    Ok(json!({
        "ok": true,
        "models": models,
        "recommended_models": recommended_models,
        "show_all": body.show_all,
        "source": if official_openai { "openai_api" } else { "openai_compatible" },
        "base_url": base_url,
    }))
}

fn normalize_base_url(raw: Option<&str>) -> String {
    let trimmed = raw.map(str::trim).unwrap_or("");
    if trimmed.is_empty() {
        "https://api.openai.com/v1".into()
    } else {
        trimmed.trim_end_matches('/').to_string()
    }
}

fn is_official_openai_host(base_url: &str) -> bool {
    base_url
        .to_ascii_lowercase()
        .contains("api.openai.com")
}

fn extract_model_ids(payload: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(items) = payload.get("data").and_then(|v| v.as_array()) {
        for item in items {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                let id = id.trim();
                if !id.is_empty() {
                    ids.push(id.to_string());
                }
            }
        }
    } else if let Some(items) = payload.as_array() {
        for item in items {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                let id = id.trim();
                if !id.is_empty() {
                    ids.push(id.to_string());
                }
            } else if let Some(id) = item.as_str() {
                let id = id.trim();
                if !id.is_empty() {
                    ids.push(id.to_string());
                }
            }
        }
    }
    ids
}

fn recommended_present_in(available: &[String]) -> Vec<String> {
    let set: std::collections::HashSet<&str> =
        available.iter().map(String::as_str).collect();
    RECOMMENDED_OPENAI_CHAT_MODELS
        .iter()
        .filter(|id| set.contains(**id))
        .map(|id| (*id).to_string())
        .collect()
}

/// Heuristic chat-completions filter for OpenAI `/v1/models` (no capability flags in list API).
/// Excludes embeddings, audio/realtime, image/video, moderation, and other non-chat families.
pub fn is_chat_completion_model(model_id: &str) -> bool {
    let id = model_id.trim().to_ascii_lowercase();
    if id.is_empty() {
        return false;
    }
    const EXCLUDE_FRAGMENTS: &[&str] = &[
        "embedding",
        "whisper",
        "tts",
        "dall-e",
        "dall·e",
        "davinci",
        "babbage",
        "moderation",
        "realtime",
        "transcribe",
        "tts-",
        "image",
        "sora",
        "audio",
        "search-preview",
        "computer-use",
        "deep-research",
        "codex",
        "gpt-oss",
    ];
    if EXCLUDE_FRAGMENTS.iter().any(|frag| id.contains(frag)) {
        return false;
    }
    // ChatGPT consumer aliases are not recommended for API subtitle use.
    if id.starts_with("chatgpt-") || id.contains("chat-latest") {
        return false;
    }
    id.starts_with("gpt-")
        || id.starts_with("o1")
        || id.starts_with("o3")
        || id.starts_with("o4")
        || id.starts_with("ft:gpt-")
        || id.starts_with("ft:o")
}

fn truncate(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        trimmed.chars().take(max_chars).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommended_list_includes_current_frontier_and_efficient_ids() {
        assert!(RECOMMENDED_OPENAI_CHAT_MODELS.contains(&"gpt-5.6-luna"));
        assert!(RECOMMENDED_OPENAI_CHAT_MODELS.contains(&"gpt-5.6-sol"));
        assert!(RECOMMENDED_OPENAI_CHAT_MODELS.contains(&"gpt-4o-mini"));
        assert!(RECOMMENDED_OPENAI_CHAT_MODELS.contains(&"gpt-4.1-mini"));
    }

    #[test]
    fn chat_filter_keeps_text_chat_models() {
        assert!(is_chat_completion_model("gpt-5.6-luna"));
        assert!(is_chat_completion_model("gpt-4o-mini"));
        assert!(is_chat_completion_model("o3-mini"));
        assert!(is_chat_completion_model("ft:gpt-4o-mini:org:custom:abc"));
    }

    #[test]
    fn chat_filter_drops_non_chat_families() {
        assert!(!is_chat_completion_model("text-embedding-3-large"));
        assert!(!is_chat_completion_model("whisper-1"));
        assert!(!is_chat_completion_model("tts-1-hd"));
        assert!(!is_chat_completion_model("dall-e-3"));
        assert!(!is_chat_completion_model("gpt-image-1"));
        assert!(!is_chat_completion_model("gpt-realtime"));
        assert!(!is_chat_completion_model("gpt-4o-transcribe"));
        assert!(!is_chat_completion_model("omni-moderation-latest"));
        assert!(!is_chat_completion_model("chatgpt-4o-latest"));
        assert!(!is_chat_completion_model("gpt-5.3-codex"));
    }

    #[test]
    fn extract_model_ids_from_openai_shape() {
        let payload = json!({
            "data": [
                {"id": "gpt-4o-mini"},
                {"id": " text-embedding-3-small "},
                {"id": ""}
            ]
        });
        assert_eq!(
            extract_model_ids(&payload),
            vec!["gpt-4o-mini".to_string(), "text-embedding-3-small".to_string()]
        );
    }
}
