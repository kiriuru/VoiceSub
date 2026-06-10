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

pub struct AzureTranslatorProvider {
    transport: Arc<SharedHttpClient>,
}

impl AzureTranslatorProvider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self { transport }
    }
}

#[async_trait]
impl TranslationProvider for AzureTranslatorProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "azure_translator",
            group: "stable",
            experimental: false,
            local_provider: false,
        }
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        let api_key = http::setting(request.settings, "api_key");
        if api_key.is_empty() {
            return Err(ProviderError::Message(
                "Azure Translator API key is missing.".into(),
            ));
        }

        let endpoint = http::setting(request.settings, "endpoint");
        let endpoint = if endpoint.is_empty() {
            "https://api.cognitive.microsofttranslator.com".to_string()
        } else {
            endpoint
        };
        if endpoint.is_empty() {
            return Err(ProviderError::Message(
                "Azure Translator endpoint is missing.".into(),
            ));
        }

        let region = http::setting(request.settings, "region");
        let source = normalize_source_lang(request.source_lang);
        let mut query = vec![("api-version", "3.0"), ("to", request.target_lang)];
        if source != "auto" {
            query.push(("from", source.as_str()));
        }

        let mut headers = vec![
            ("Ocp-Apim-Subscription-Key", api_key.as_str()),
            ("Content-Type", "application/json"),
        ];
        if !region.is_empty() {
            headers.push(("Ocp-Apim-Subscription-Region", region.as_str()));
        }

        let url = format!("{}/translate", endpoint.trim_end_matches('/'));
        let payload = http::request_json(
            &self.transport.client(),
            Method::POST,
            &url,
            Some(&query),
            Some(&json!([{ "Text": request.text }])),
            None,
            Some(&headers),
            "Azure Translator request failed",
        )
        .await?;

        let translated = payload
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item.get("translations"))
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .and_then(|item| item.get("text"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if translated.is_empty() {
            return Err(ProviderError::Message(
                "Azure Translator returned an empty translation.".into(),
            ));
        }
        Ok(translated)
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        base_diagnostics(&self.info(), settings)
    }
}
