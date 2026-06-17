use std::sync::{Arc, RwLock};
use std::time::Duration;

use reqwest::{Client, Method};
use serde_json::Value;

use super::{
    DEFAULT_HTTP_CONNECT_TIMEOUT_SECONDS, DEFAULT_HTTP_KEEPALIVE_EXPIRY_SECONDS,
    DEFAULT_HTTP_KEEPALIVE_LIMIT, DEFAULT_REQUEST_TIMEOUT_SECONDS, ProviderError,
};

/// Shared translation HTTP client (SST `TranslationEngine._get_or_create_http_client` parity).
pub fn build_translation_http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs_f64(DEFAULT_REQUEST_TIMEOUT_SECONDS))
        .connect_timeout(Duration::from_secs(DEFAULT_HTTP_CONNECT_TIMEOUT_SECONDS))
        .pool_max_idle_per_host(DEFAULT_HTTP_KEEPALIVE_LIMIT)
        .pool_idle_timeout(Duration::from_secs(DEFAULT_HTTP_KEEPALIVE_EXPIRY_SECONDS))
        .build()
        .unwrap_or_else(|_| Client::new())
}

pub type HttpClientProvider = Arc<dyn Fn() -> Client + Send + Sync>;

pub struct SharedHttpClient {
    fallback: Client,
    provider: RwLock<Option<HttpClientProvider>>,
}

impl SharedHttpClient {
    pub fn new(fallback: Client) -> Arc<Self> {
        Arc::new(Self {
            fallback,
            provider: RwLock::new(None),
        })
    }

    pub fn bind(&self, provider: HttpClientProvider) {
        if let Ok(mut slot) = self.provider.write() {
            *slot = Some(provider);
        }
    }

    pub fn is_bound(&self) -> bool {
        self.provider
            .read()
            .ok()
            .and_then(|slot| slot.as_ref().map(|_| true))
            .unwrap_or(false)
    }

    pub fn client(&self) -> Client {
        if let Ok(slot) = self.provider.read()
            && let Some(provider) = slot.as_ref()
        {
            return provider();
        }
        self.fallback.clone()
    }
}

pub fn setting(settings: &std::collections::HashMap<String, String>, key: &str) -> String {
    settings
        .get(key)
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
pub async fn request_json(
    client: &Client,
    method: Method,
    url: &str,
    query: Option<&[(&str, &str)]>,
    json_body: Option<&Value>,
    form: Option<&[(&str, &str)]>,
    headers: Option<&[(&str, &str)]>,
    error_prefix: &str,
) -> Result<Value, ProviderError> {
    let mut request = client.request(method, url);
    if let Some(query_params) = query {
        request = request.query(query_params);
    }
    if let Some(header_pairs) = headers {
        for (name, value) in header_pairs {
            request = request.header(*name, *value);
        }
    }
    if let Some(form_fields) = form {
        request = request.form(form_fields);
    } else if let Some(body) = json_body {
        request = request.json(body);
    }

    let response = request.send().await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let detail = body.trim();
        let suffix = if detail.is_empty() {
            String::new()
        } else {
            let truncated = if detail.chars().count() > 280 {
                detail.chars().take(280).collect::<String>()
            } else {
                detail.to_string()
            };
            format!(" - {truncated}")
        };
        return Err(ProviderError::Message(format!(
            "{error_prefix}: HTTP {status}{suffix}"
        )));
    }

    Ok(response.json().await?)
}

pub fn html_unescape(value: &str) -> String {
    let mut out = value.to_string();
    for (entity, ch) in [
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&apos;", "'"),
    ] {
        out = out.replace(entity, ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_translation_http_client_is_reusable() {
        let client = build_translation_http_client();
        assert!(
            client
                .get("https://translation.googleapis.com")
                .build()
                .is_ok()
        );
    }
}
