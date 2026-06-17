use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::{Value, json};

use std::sync::Arc;

use super::{
    ProviderError, ProviderInfo, TranslateRequest, TranslationProvider, base_diagnostics,
    http::SharedHttpClient, mask_secret, normalize_source_lang,
};

pub struct GoogleTranslateV2Provider {
    transport: Arc<SharedHttpClient>,
}

impl GoogleTranslateV2Provider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self { transport }
    }

    fn normalize_api_key(raw: &str) -> (String, Value) {
        let trimmed = raw.trim();
        let mut normalized = trimmed.to_string();
        let mut extracted_from_query = false;
        let mut removed_trailing_query = false;

        if trimmed.contains("key=")
            && let Some(start) = trimmed.find("key=")
        {
            let query = &trimmed[start..];
            if let Some(value) = query.strip_prefix("key=") {
                let candidate = value.split('&').next().unwrap_or(value).trim();
                if !candidate.is_empty() {
                    normalized = candidate.to_string();
                    extracted_from_query = candidate != trimmed;
                }
            }
        }

        if normalized.starts_with("AIza")
            && normalized.contains('&')
            && let Some(candidate) = normalized.split('&').next()
            && !candidate.is_empty()
            && candidate != normalized
        {
            normalized = candidate.to_string();
            removed_trailing_query = true;
        }

        let diagnostics = json!({
            "api_key_present": !normalized.is_empty(),
            "api_key_length": normalized.len(),
            "api_key_masked_preview": mask_secret(&normalized),
            "api_key_trimmed_changed": raw != trimmed,
            "api_key_sanitized_changed": trimmed != normalized,
            "api_key_extracted_from_query": extracted_from_query,
            "api_key_removed_trailing_query": removed_trailing_query,
        });
        (normalized, diagnostics)
    }
}

#[async_trait]
impl TranslationProvider for GoogleTranslateV2Provider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "google_translate_v2",
            group: "stable",
            experimental: false,
            local_provider: false,
        }
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        let (api_key, _) = Self::normalize_api_key(
            request
                .settings
                .get("api_key")
                .map(String::as_str)
                .unwrap_or(""),
        );
        if api_key.is_empty() {
            return Err(ProviderError::Message(
                "Google Translate v2 API key is missing.".into(),
            ));
        }

        let source = normalize_source_lang(request.source_lang);
        let mut form: Vec<(&str, &str)> = vec![
            ("q", request.text),
            ("target", request.target_lang),
            ("format", "text"),
        ];
        let source_ref = if source != "auto" {
            Some(source.as_str())
        } else {
            None
        };
        if let Some(source_lang) = source_ref {
            form.push(("source", source_lang));
        }

        let response = self
            .transport
            .client()
            .post("https://translation.googleapis.com/language/translate/v2")
            .query(&[("key", api_key.as_str())])
            .form(&form)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Message(format!(
                "Google Translate v2 HTTP {status}: {body}"
            )));
        }

        let payload: Value = response.json().await?;
        let translated = payload
            .get("data")
            .and_then(|v| v.get("translations"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("translatedText"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if translated.is_empty() {
            return Err(ProviderError::Message(
                "Google Translate v2 returned empty translation.".into(),
            ));
        }
        Ok(super::http::html_unescape(&translated))
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        let (api_key, key_diag) =
            Self::normalize_api_key(settings.get("api_key").map(String::as_str).unwrap_or(""));
        let mut diag = base_diagnostics(&self.info(), settings);
        if let Some(obj) = diag.as_object_mut() {
            for (k, v) in key_diag.as_object().unwrap_or(&serde_json::Map::new()) {
                obj.insert(k.clone(), v.clone());
            }
            obj.insert("api_key_present".into(), json!(!api_key.is_empty()));
        }
        diag
    }
}
