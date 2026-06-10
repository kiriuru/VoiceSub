use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{json, Value};

use std::sync::Arc;

use super::{
    http::SharedHttpClient,
    base_diagnostics, http, normalize_source_lang, ProviderError, ProviderInfo, TranslateRequest,
    TranslationProvider,
};

pub struct LibreTranslateProvider {
    transport: Arc<SharedHttpClient>,
}

impl LibreTranslateProvider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self { transport }
    }
}

#[async_trait]
impl TranslationProvider for LibreTranslateProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "libretranslate",
            group: "stable",
            experimental: false,
            local_provider: false,
        }
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        let api_url = http::setting(request.settings, "api_url");
        let api_url = if api_url.is_empty() {
            "https://libretranslate.com/translate"
        } else {
            api_url.as_str()
        };

        let mut body = json!({
            "q": request.text,
            "source": normalize_source_lang(request.source_lang),
            "target": request.target_lang,
            "format": "text",
        });
        let api_key = http::setting(request.settings, "api_key");
        if !api_key.is_empty() {
            body["api_key"] = json!(api_key);
        }

        let payload = http::request_json(
            &self.transport.client(),
            Method::POST,
            api_url,
            None,
            Some(&body),
            None,
            None,
            "LibreTranslate request failed",
        )
        .await?;

        let translated = payload
            .get("translatedText")
            .or_else(|| payload.get("translation"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if translated.is_empty() {
            return Err(ProviderError::Message(
                "LibreTranslate returned an empty translation.".into(),
            ));
        }
        Ok(translated)
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        base_diagnostics(&self.info(), settings)
    }
}
