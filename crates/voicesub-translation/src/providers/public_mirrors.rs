use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Value, json};

use std::sync::Arc;

use super::{
    ProviderError, ProviderInfo, TranslateRequest, TranslationProvider, base_diagnostics, http,
    http::SharedHttpClient, normalize_source_lang,
};

pub struct PublicLibreTranslateMirrorProvider {
    transport: Arc<SharedHttpClient>,
}

impl PublicLibreTranslateMirrorProvider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self { transport }
    }
}

#[async_trait]
impl TranslationProvider for PublicLibreTranslateMirrorProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "public_libretranslate_mirror",
            group: "experimental",
            experimental: true,
            local_provider: false,
        }
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        let api_url = http::setting(request.settings, "api_url");
        let api_url = if api_url.is_empty() {
            "https://translate.fedilab.app/translate"
        } else {
            api_url.as_str()
        };

        let body = json!({
            "q": request.text,
            "source": normalize_source_lang(request.source_lang),
            "target": request.target_lang,
            "format": "text",
        });

        let payload = http::request_json(
            &self.transport.client(),
            Method::POST,
            api_url,
            None,
            Some(&body),
            None,
            None,
            "Public LibreTranslate mirror request failed",
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
                "Public LibreTranslate mirror returned an empty translation.".into(),
            ));
        }
        Ok(translated)
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        let mut diag = base_diagnostics(&self.info(), settings);
        if let Some(obj) = diag.as_object_mut() {
            obj.insert(
                "status_message".into(),
                json!("Experimental public LibreTranslate mirror. Availability may change."),
            );
        }
        diag
    }
}
