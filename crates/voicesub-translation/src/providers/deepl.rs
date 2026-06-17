use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::Value;

use std::sync::Arc;

use super::{
    ProviderError, ProviderInfo, TranslateRequest, TranslationProvider, base_diagnostics, http,
    http::SharedHttpClient, normalize_source_lang,
};

pub struct DeepLProvider {
    transport: Arc<SharedHttpClient>,
}

impl DeepLProvider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self { transport }
    }
}

#[async_trait]
impl TranslationProvider for DeepLProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "deepl",
            group: "stable",
            experimental: false,
            local_provider: false,
        }
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        let api_key = http::setting(request.settings, "api_key");
        if api_key.is_empty() {
            return Err(ProviderError::Message("DeepL API key is missing.".into()));
        }

        let configured_api_url = http::setting(request.settings, "api_url");
        let api_url = if configured_api_url.is_empty() {
            "https://api-free.deepl.com/v2/translate".to_string()
        } else {
            configured_api_url
        };

        let target_lang = request.target_lang.to_ascii_uppercase();
        let source = normalize_source_lang(request.source_lang);
        let upper_source = if source != "auto" {
            Some(source.to_ascii_uppercase())
        } else {
            None
        };
        let mut form: Vec<(&str, &str)> = vec![
            ("auth_key", api_key.as_str()),
            ("text", request.text),
            ("target_lang", target_lang.as_str()),
        ];
        if let Some(ref source_lang) = upper_source {
            form.push(("source_lang", source_lang.as_str()));
        }

        let payload = http::request_json(
            &self.transport.client(),
            Method::POST,
            api_url.as_str(),
            None,
            None,
            Some(&form),
            None,
            "DeepL request failed",
        )
        .await?;

        let translated = payload
            .get("translations")
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .and_then(|item| item.get("text"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if translated.is_empty() {
            return Err(ProviderError::Message(
                "DeepL returned an empty translation.".into(),
            ));
        }
        Ok(translated)
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        base_diagnostics(&self.info(), settings)
    }
}
