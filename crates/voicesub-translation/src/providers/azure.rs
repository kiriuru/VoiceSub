use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Value, json};

use std::sync::Arc;

use super::{
    ProviderError, ProviderInfo, TranslateRequest, TranslationProvider, base_diagnostics, http,
    http::SharedHttpClient, lang_codes::azure_lang, normalize_source_lang,
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
        let target = azure_lang(request.target_lang);
        let from = if source == "auto" {
            None
        } else {
            Some(azure_lang(&source))
        };
        let mut query = vec![("api-version", "3.0"), ("to", target.as_str())];
        if let Some(ref from_lang) = from {
            query.push(("from", from_lang.as_str()));
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
            request.timeout_secs,
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
        let region = http::setting(settings, "region");
        let endpoint = http::setting(settings, "endpoint");
        let mut diag = base_diagnostics(&self.info(), settings);
        if let Some(obj) = diag.as_object_mut() {
            obj.insert("region_present".into(), json!(!region.trim().is_empty()));
            if region.trim().is_empty() {
                obj.insert(
                    "status_message".into(),
                    json!(
                        "Azure region is empty. Multi-service / regional keys usually require Ocp-Apim-Subscription-Region when using the global endpoint."
                    ),
                );
                obj.insert("region_missing_warning".into(), json!(true));
            } else {
                obj.insert(
                    "status_message".into(),
                    json!(format!(
                        "Azure Translator via {} (region={region}).",
                        if endpoint.is_empty() {
                            "https://api.cognitive.microsofttranslator.com"
                        } else {
                            endpoint.as_str()
                        }
                    )),
                );
            }
        }
        diag
    }
}
