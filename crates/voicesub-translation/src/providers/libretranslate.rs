use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Value, json};

use std::sync::Arc;

use super::{
    ProviderError, ProviderInfo, TranslateRequest, TranslationProvider, base_diagnostics, http,
    http::SharedHttpClient,
    lang_codes::libretranslate_lang,
    normalize_source_lang,
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

        let source = normalize_source_lang(request.source_lang);
        let source = if source == "auto" {
            "auto".to_string()
        } else {
            libretranslate_lang(&source)
        };
        let mut body = json!({
            "q": request.text,
            "source": source,
            "target": libretranslate_lang(request.target_lang),
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
            request.timeout_secs,
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
