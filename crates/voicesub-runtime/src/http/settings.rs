use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use voicesub_config::{build_font_catalog, normalize_config_payload, read_full_logging_enabled};
use voicesub_logging::apply_logging_preferences;

use super::state::HttpState;

#[derive(Debug, Deserialize)]
pub struct SettingsSaveRequest {
    #[serde(default)]
    pub payload: Value,
}

pub async fn settings_load(State(state): State<Arc<HttpState>>) -> impl IntoResponse {
    let store = state.config.read().await;
    let payload = normalize_config_payload(store.payload().clone());
    let subtitle_style = payload.get("subtitle_style");
    let subtitle_style_presets = (state.style_presets)(subtitle_style);
    let font_catalog = build_font_catalog(&state.paths.fonts_dir);
    Json(json!({
        "ok": true,
        "loaded_from": store.document().loaded_from(),
        "payload": payload,
        "subtitle_style_presets": subtitle_style_presets,
        "font_catalog": font_catalog
    }))
}

pub async fn settings_save(
    State(state): State<Arc<HttpState>>,
    Json(body): Json<SettingsSaveRequest>,
) -> Response {
    let saved = {
        let mut store = state.config.write().await;
        match store.apply_save_payload(&body.payload) {
            Ok(()) => {
                if let Ok(mut snapshot) = state.config_snapshot.write() {
                    *snapshot = store.payload().clone();
                }
                Ok(store.payload().clone())
            }
            Err(err) => Err(err.to_string()),
        }
    };

    let payload = match saved {
        Ok(payload) => payload,
        Err(message) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "message": message })),
            )
                .into_response();
        }
    };

    apply_logging_preferences(&state.paths.logs_dir, read_full_logging_enabled(&payload));
    state.translation.lock().await.apply_live_settings().await;
    state.obs_captions.apply_live_settings().await;
    state.subtitle.republish_latest().await;
    let subtitle_style = payload.get("subtitle_style");
    let subtitle_style_presets = (state.style_presets)(subtitle_style);
    let font_catalog = build_font_catalog(&state.paths.fonts_dir);
    Json(json!({
        "ok": true,
        "message": "saved",
        "payload": payload,
        "subtitle_style_presets": subtitle_style_presets,
        "font_catalog": font_catalog,
        "live_applied": true
    }))
    .into_response()
}
