use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::{debug, warn};
use voicesub_logging::tts_trace;
use voicesub_tts::{GOOGLE_TTS_MAX_CHARS, fetch_google_tts_browser, normalize_tts_lang};

#[derive(Debug, Deserialize)]
pub struct GoogleTtsQuery {
    pub q: String,
    #[serde(default = "default_lang")]
    pub tl: String,
}

fn default_lang() -> String {
    "en".to_string()
}

pub fn validate_tts_query(params: &GoogleTtsQuery) -> Result<&str, StatusCode> {
    let text = params.q.trim();
    if text.is_empty() {
        tts_trace(
            "proxy",
            "reject_empty",
            serde_json::json!({ "tl": params.tl }),
        );
        return Err(StatusCode::BAD_REQUEST);
    }
    if text.chars().count() > GOOGLE_TTS_MAX_CHARS {
        tts_trace(
            "proxy",
            "reject_too_long",
            serde_json::json!({
                "tl": params.tl,
                "text_len": text.chars().count(),
                "max_chars": GOOGLE_TTS_MAX_CHARS,
            }),
        );
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(text)
}

pub fn audio_mpeg_response(bytes: Vec<u8>) -> Response {
    use axum::http::header;
    match Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "audio/mpeg")
        .header(header::CACHE_CONTROL, "no-store")
        .body(axum::body::Body::from(bytes))
    {
        Ok(response) => response,
        Err(err) => {
            warn!(target: "voicesub.tts.proxy", error = %err, "response build failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Local same-origin proxy for Google Translate TTS (WebView2 referer workaround).
pub async fn google_tts_proxy(Query(params): Query<GoogleTtsQuery>) -> impl IntoResponse {
    let text = match validate_tts_query(&params) {
        Ok(text) => text,
        Err(status) => return status.into_response(),
    };

    let tl = normalize_tts_lang(&params.tl);
    debug!(
        target: "voicesub.tts.proxy",
        tl = %tl,
        text_len = text.chars().count(),
        "fetching google tts"
    );

    let bytes = match fetch_google_tts_browser(&tl, text).await {
        Ok(bytes) => bytes,
        Err(err) => {
            warn!(target: "voicesub.tts.proxy", error = %err, "upstream request failed");
            tts_trace(
                "proxy",
                "upstream_fetch_error",
                serde_json::json!({
                    "tl": tl,
                    "text_len": text.chars().count(),
                    "message": err,
                }),
            );
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    tts_trace(
        "proxy",
        "ok",
        serde_json::json!({
            "tl": tl,
            "text_len": text.chars().count(),
            "bytes": bytes.len(),
        }),
    );

    audio_mpeg_response(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_empty_query() {
        let params = GoogleTtsQuery {
            q: "   ".into(),
            tl: "en".into(),
        };
        assert_eq!(validate_tts_query(&params), Err(StatusCode::BAD_REQUEST));
    }

    #[test]
    fn validate_rejects_too_long_text_by_unicode_scalar_count() {
        let params = GoogleTtsQuery {
            q: "😀".repeat(GOOGLE_TTS_MAX_CHARS + 1),
            tl: "en".into(),
        };
        assert_eq!(params.q.chars().count(), GOOGLE_TTS_MAX_CHARS + 1);
        assert_eq!(validate_tts_query(&params), Err(StatusCode::BAD_REQUEST));
    }

    #[test]
    fn validate_accepts_text_at_limit() {
        let params = GoogleTtsQuery {
            q: "a".repeat(GOOGLE_TTS_MAX_CHARS),
            tl: "ru".into(),
        };
        assert_eq!(validate_tts_query(&params), Ok(params.q.as_str()));
    }
}
