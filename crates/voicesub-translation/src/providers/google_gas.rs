use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Value, json};

use std::sync::Arc;

use super::{
    ProviderError, ProviderInfo, TranslateRequest, TranslationProvider, base_diagnostics, http,
    http::SharedHttpClient, normalize_source_lang,
};

pub struct GoogleGasUrlProvider {
    transport: Arc<SharedHttpClient>,
}

impl GoogleGasUrlProvider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self { transport }
    }
}

#[async_trait]
impl TranslationProvider for GoogleGasUrlProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "google_gas_url",
            group: "experimental",
            experimental: true,
            local_provider: false,
        }
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        let gas_url = http::setting(request.settings, "gas_url");
        if gas_url.is_empty() {
            return Err(ProviderError::Message("Google GAS URL is missing.".into()));
        }

        let body = json!({
            "text": request.text,
            "source_lang": normalize_source_lang(request.source_lang),
            "target_lang": request.target_lang,
        });

        let payload = http::request_json(
            &self.transport.client(),
            Method::POST,
            &gas_url,
            None,
            Some(&body),
            None,
            None,
            "Google GAS URL request failed",
            request.timeout_secs,
        )
        .await?;

        let translated = ["translatedText", "text", "translation", "output"]
            .iter()
            .find_map(|key| payload.get(*key))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if translated.is_empty() {
            return Err(ProviderError::Message(
                "Google GAS URL returned no translated text. Expected one of: translatedText, text, translation, output.".into(),
            ));
        }

        Ok(translated)
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        let mut diag = base_diagnostics(&self.info(), settings);
        if let Some(obj) = diag.as_object_mut() {
            obj.insert(
                "status_message".into(),
                json!("Experimental Google GAS URL provider. Reliability depends on your script."),
            );
        }
        diag
    }
}
