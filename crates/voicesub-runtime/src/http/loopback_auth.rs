//! Per-session secret for `/api/*` — blocks cross-origin browser CSRF/exfiltration.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use super::state::HttpState;

pub const LOOPBACK_TOKEN_HEADER: &str = "x-voicesub-token";

#[derive(Clone)]
pub struct LoopbackAuth {
    token: Arc<str>,
}

impl LoopbackAuth {
    pub fn generate() -> Self {
        Self {
            token: Arc::from(Uuid::new_v4().to_string()),
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn authorize_headers(&self, headers: &HeaderMap) -> bool {
        if let Some(value) = headers.get(LOOPBACK_TOKEN_HEADER)
            && let Ok(provided) = value.to_str()
            && constant_time_eq(provided.as_bytes(), self.token.as_bytes())
        {
            return true;
        }
        false
    }

    pub fn inject_token_script(&self, html: &str) -> String {
        let script = format!(
            "<script>window.__VOICESUB_API_TOKEN__={};</script>",
            serde_json::to_string(self.token()).unwrap_or_default()
        );
        if let Some(pos) = html.find("</head>") {
            let mut out = String::with_capacity(html.len() + script.len());
            out.push_str(&html[..pos]);
            out.push_str(&script);
            out.push_str(&html[pos..]);
            out
        } else {
            format!("{script}{html}")
        }
    }
}

pub async fn loopback_auth_middleware(
    State(state): State<Arc<HttpState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if state.loopback_auth.authorize_headers(request.headers()) {
        return Ok(next.run(request).await);
    }

    tracing::warn!(
        path = %request.uri().path(),
        "loopback API request rejected: missing or invalid token"
    );
    Err(StatusCode::UNAUTHORIZED)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn authorize_accepts_matching_header() {
        let auth = LoopbackAuth::generate();
        let mut headers = HeaderMap::new();
        headers.insert(
            LOOPBACK_TOKEN_HEADER,
            HeaderValue::from_str(auth.token()).expect("token header"),
        );
        assert!(auth.authorize_headers(&headers));
    }

    #[test]
    fn authorize_rejects_missing_and_wrong_token() {
        let auth = LoopbackAuth::generate();
        assert!(!auth.authorize_headers(&HeaderMap::new()));
        let mut headers = HeaderMap::new();
        headers.insert(LOOPBACK_TOKEN_HEADER, HeaderValue::from_static("wrong"));
        assert!(!auth.authorize_headers(&headers));
    }

    #[test]
    fn inject_token_script_inserts_before_head_close() {
        let auth = LoopbackAuth::generate();
        let html = "<html><head><title>x</title></head><body></body></html>";
        let out = auth.inject_token_script(html);
        assert!(out.contains("__VOICESUB_API_TOKEN__"));
        assert!(out.find("</head>").unwrap() > out.find("__VOICESUB_API_TOKEN__").unwrap());
    }
}
