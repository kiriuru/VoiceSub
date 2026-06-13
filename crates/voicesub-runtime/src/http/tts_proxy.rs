use std::sync::OnceLock;
use std::time::Duration;

use axum::{
    extract::Query,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::{debug, warn};
use urlencoding::encode;
use voicesub_logging::tts_trace;

pub const GOOGLE_TTS_MAX_CHARS: usize = 200;

#[derive(Debug, Deserialize)]
pub struct GoogleTtsQuery {
    pub q: String,
    #[serde(default = "default_lang")]
    pub tl: String,
}

fn default_lang() -> String {
    "en".to_string()
}

pub fn normalize_lang(lang: &str) -> String {
    let trimmed = lang.trim().to_lowercase();
    if trimmed.is_empty() {
        return "en".to_string();
    }
    trimmed
        .split('-')
        .next()
        .unwrap_or("en")
        .split('_')
        .next()
        .unwrap_or("en")
        .to_string()
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

fn google_tts_http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .pool_max_idle_per_host(8)
            .tcp_keepalive(Duration::from_secs(30))
            .build()
            .expect("google tts reqwest client")
    })
}

fn google_upstream_url(text: &str, lang: &str) -> String {
    let tl = normalize_lang(lang);
    let textlen = text.chars().count();
    format!(
        "https://translate.google.com/translate_tts?ie=UTF-8&client=tw-ob&tl={tl}&q={q}&total=1&idx=0&textlen={textlen}",
        tl = tl,
        q = encode(text),
        textlen = textlen
    )
}

/// Local same-origin proxy for Google Translate TTS (WebView2 referer workaround).
pub async fn google_tts_proxy(Query(params): Query<GoogleTtsQuery>) -> impl IntoResponse {
    let text = match validate_tts_query(&params) {
        Ok(text) => text,
        Err(status) => return status.into_response(),
    };

    let upstream = google_upstream_url(text, &params.tl);
    debug!(
        target: "voicesub.tts.proxy",
        tl = %normalize_lang(&params.tl),
        text_len = text.chars().count(),
        "fetching google tts"
    );

    let response = match google_tts_http_client()
        .get(&upstream)
        .header(header::REFERER, "https://translate.google.com/")
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            warn!(target: "voicesub.tts.proxy", error = %err, "upstream request failed");
            tts_trace(
                "proxy",
                "upstream_fetch_error",
                serde_json::json!({
                    "tl": normalize_lang(&params.tl),
                    "text_len": text.chars().count(),
                    "message": err.to_string(),
                }),
            );
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    let status = response.status();
    if !status.is_success() {
        warn!(
            target: "voicesub.tts.proxy",
            status = %status,
            "upstream non-success"
        );
        tts_trace(
            "proxy",
            "upstream_http_error",
            serde_json::json!({
                "tl": normalize_lang(&params.tl),
                "text_len": text.chars().count(),
                "status": status.as_u16(),
            }),
        );
        return StatusCode::BAD_GATEWAY.into_response();
    }

    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(err) => {
            warn!(target: "voicesub.tts.proxy", error = %err, "upstream body read failed");
            tts_trace(
                "proxy",
                "upstream_body_error",
                serde_json::json!({ "message": err.to_string() }),
            );
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    if bytes.is_empty() {
        warn!(target: "voicesub.tts.proxy", "upstream empty audio body");
        tts_trace(
            "proxy",
            "upstream_empty_body",
            serde_json::json!({
                "tl": normalize_lang(&params.tl),
                "text_len": text.chars().count(),
            }),
        );
        return StatusCode::BAD_GATEWAY.into_response();
    }

    tts_trace(
        "proxy",
        "ok",
        serde_json::json!({
            "tl": normalize_lang(&params.tl),
            "text_len": text.chars().count(),
            "bytes": bytes.len(),
        }),
    );

    audio_mpeg_response(bytes.to_vec())
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

    #[test]
    fn normalize_lang_strips_region() {
        assert_eq!(normalize_lang("ru-RU"), "ru");
        assert_eq!(normalize_lang(""), "en");
    }
}
