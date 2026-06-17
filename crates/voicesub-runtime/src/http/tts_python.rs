use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use tracing::{debug, warn};
use voicesub_logging::tts_trace;
use voicesub_tts::{
    PythonRuntimeKind, PythonRuntimeStatus, probe_python_runtime, run_google_tts_fetch,
};

use super::state::HttpState;
use super::tts_proxy::{GoogleTtsQuery, audio_mpeg_response, validate_tts_query};

pub async fn python_tts_status(
    State(state): State<std::sync::Arc<HttpState>>,
) -> Json<PythonRuntimeStatus> {
    Json(probe_python_runtime(&state.paths.tts_module_dir()).await)
}

/// Google TTS via module-embedded Python fetcher (Nuitka onefile) or dev system fallback.
pub async fn python_tts_proxy(
    State(state): State<std::sync::Arc<HttpState>>,
    Query(params): Query<GoogleTtsQuery>,
) -> impl IntoResponse {
    let text = match validate_tts_query(&params) {
        Ok(text) => text,
        Err(status) => return status.into_response(),
    };

    let tl = voicesub_tts::normalize_tts_lang(&params.tl);
    debug!(
        target: "voicesub.tts.python",
        tl = %tl,
        text_len = text.chars().count(),
        "python tts fetch"
    );

    match run_google_tts_fetch(&state.paths.tts_module_dir(), &tl, text).await {
        Ok((bytes, kind)) => {
            let kind_label = match kind {
                PythonRuntimeKind::Embedded => "embedded",
                PythonRuntimeKind::SystemScript => "system_dev_fallback",
            };
            tts_trace(
                "python",
                "ok",
                serde_json::json!({
                    "tl": tl,
                    "text_len": text.chars().count(),
                    "bytes": bytes.len(),
                    "runtime_kind": kind_label,
                }),
            );
            audio_mpeg_response(bytes)
        }
        Err(message) => {
            warn!(target: "voicesub.tts.python", error = %message, "python tts failed");
            tts_trace(
                "python",
                "error",
                serde_json::json!({
                    "tl": tl,
                    "text_len": text.chars().count(),
                    "message": message,
                }),
            );
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}
