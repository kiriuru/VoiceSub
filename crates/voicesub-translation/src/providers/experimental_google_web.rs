use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Value, json};

use std::sync::Arc;

use super::{
    ProviderError, ProviderInfo, TranslateRequest, TranslationProvider, base_diagnostics, http,
    http::SharedHttpClient, normalize_source_lang,
};

fn build_google_translate_url(text: &str, source_lang: &str, target_lang: &str) -> String {
    format!(
        "https://translate.googleapis.com/translate_a/single?client=gtx&sl={}&tl={}&dt=t&q={}",
        urlencoding::encode(source_lang),
        urlencoding::encode(target_lang),
        urlencoding::encode(text),
    )
}

fn extract_google_translation_text(payload: &Value) -> String {
    let mut translated_parts = Vec::new();
    if let Some(first) = payload.as_array().and_then(|items| items.first())
        && let Some(chunks) = first.as_array()
    {
        for item in chunks {
            if let Some(parts) = item.as_array()
                && let Some(text) = parts.first().and_then(|value| value.as_str())
            {
                translated_parts.push(text);
            }
        }
    }
    translated_parts.join("").trim().to_string()
}

struct GoogleWebLikeProvider {
    transport: Arc<SharedHttpClient>,
    info: ProviderInfo,
    error_prefix: &'static str,
    status_message: &'static str,
}

impl GoogleWebLikeProvider {
    async fn translate_inner(
        &self,
        request: TranslateRequest<'_>,
    ) -> Result<String, ProviderError> {
        let source = normalize_source_lang(request.source_lang);
        let url = build_google_translate_url(request.text, &source, request.target_lang);
        let payload = http::request_json(
            &self.transport.client(),
            Method::GET,
            &url,
            None,
            None,
            None,
            None,
            self.error_prefix,
        )
        .await?;

        let translated = extract_google_translation_text(&payload);
        if translated.is_empty() {
            return Err(ProviderError::Message(format!(
                "{} returned an empty translation.",
                self.error_prefix
            )));
        }
        Ok(translated)
    }

    fn diagnostics_inner(&self, settings: &HashMap<String, String>) -> Value {
        let mut diag = base_diagnostics(&self.info, settings);
        if let Some(obj) = diag.as_object_mut() {
            obj.insert("status_message".into(), json!(self.status_message));
        }
        diag
    }
}

pub struct GoogleWebProvider {
    inner: GoogleWebLikeProvider,
}

impl GoogleWebProvider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self {
            inner: GoogleWebLikeProvider {
                transport,
                info: ProviderInfo {
                    name: "google_web",
                    group: "experimental",
                    experimental: true,
                    local_provider: false,
                },
                error_prefix: "Google Web request failed",
                status_message: "Experimental Google Web provider. Best-effort only.",
            },
        }
    }
}

#[async_trait]
impl TranslationProvider for GoogleWebProvider {
    fn info(&self) -> ProviderInfo {
        self.inner.info
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        self.inner.translate_inner(request).await
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        self.inner.diagnostics_inner(settings)
    }
}

pub struct FreeWebTranslateProvider {
    inner: GoogleWebLikeProvider,
}

impl FreeWebTranslateProvider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self {
            inner: GoogleWebLikeProvider {
                transport,
                info: ProviderInfo {
                    name: "free_web_translate",
                    group: "experimental",
                    experimental: true,
                    local_provider: false,
                },
                error_prefix: "Free Web Translate request failed",
                status_message: "Experimental free web provider. Best-effort only.",
            },
        }
    }
}

#[async_trait]
impl TranslationProvider for FreeWebTranslateProvider {
    fn info(&self) -> ProviderInfo {
        self.inner.info
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        self.inner.translate_inner(request).await
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        self.inner.diagnostics_inner(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_google_translation_text_joins_chunks() {
        let payload = json!([
            [["Hello ", null, null, null], ["world", null, null, null],],
            null,
            "en",
        ]);
        assert_eq!(extract_google_translation_text(&payload), "Hello world");
    }
}
