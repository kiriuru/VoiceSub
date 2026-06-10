use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{json, Value};

use std::sync::Arc;

use super::{
    http::SharedHttpClient,
    base_diagnostics, http, mask_secret, normalize_source_lang, ProviderError, ProviderInfo,
    TranslateRequest, TranslationProvider,
};

pub struct GoogleCloudTranslationV3Provider {
    transport: Arc<SharedHttpClient>,
    api_root: String,
}

impl GoogleCloudTranslationV3Provider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self {
            transport,
            api_root: "https://translation.googleapis.com".into(),
        }
    }

    #[doc(hidden)]
    pub fn with_api_root_for_test(mut self, api_root: impl Into<String>) -> Self {
        self.api_root = api_root.into();
        self
    }
}

#[async_trait]
impl TranslationProvider for GoogleCloudTranslationV3Provider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "google_cloud_translation_v3",
            group: "stable",
            experimental: false,
            local_provider: false,
        }
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        let project_id = http::setting(request.settings, "project_id");
        if project_id.is_empty() {
            return Err(ProviderError::Message(
                "Google Cloud Translation v3 project ID is missing.".into(),
            ));
        }

        let access_token = http::setting(request.settings, "access_token");
        if access_token.is_empty() {
            return Err(ProviderError::Message(
                "Google Cloud Translation v3 access token is missing.".into(),
            ));
        }

        let location = http::setting(request.settings, "location");
        let location = if location.is_empty() {
            "global"
        } else {
            location.as_str()
        };
        let model = http::setting(request.settings, "model");

        let endpoint = format!(
            "{}/v3/projects/{project_id}/locations/{location}:translateText",
            self.api_root.trim_end_matches('/')
        );

        let source = normalize_source_lang(request.source_lang);
        let mut body = json!({
            "contents": [request.text],
            "targetLanguageCode": request.target_lang,
            "mimeType": "text/plain",
        });
        if source != "auto" {
            body["sourceLanguageCode"] = json!(source);
        }
        if !model.is_empty() {
            body["model"] = json!(model);
        }

        let headers = [
            (
                "Authorization".to_string(),
                format!("Bearer {access_token}"),
            ),
            (
                "Content-Type".to_string(),
                "application/json; charset=utf-8".to_string(),
            ),
            ("x-goog-user-project".to_string(), project_id.clone()),
        ];
        let header_refs: Vec<(&str, &str)> = headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();

        let client = self.transport.client();
        let payload = http::request_json(
            &client,
            Method::POST,
            &endpoint,
            None,
            Some(&body),
            None,
            Some(&header_refs),
            "Google Cloud Translation v3 request failed",
        )
        .await?;

        let translated = payload
            .get("translations")
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .and_then(|item| item.get("translatedText"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if translated.is_empty() {
            return Err(ProviderError::Message(
                "Google Cloud Translation v3 returned an empty translation.".into(),
            ));
        }
        Ok(http::html_unescape(&translated))
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        let project_id = http::setting(settings, "project_id");
        let access_token = http::setting(settings, "access_token");
        let location = http::setting(settings, "location");
        let location = if location.is_empty() {
            "global".to_string()
        } else {
            location
        };
        let model = http::setting(settings, "model");

        let endpoint = if project_id.is_empty() {
            String::new()
        } else {
            format!(
                "https://translation.googleapis.com/v3/projects/{project_id}/locations/{location}:translateText"
            )
        };

        let mut diag = base_diagnostics(&self.info(), settings);
        if let Some(obj) = diag.as_object_mut() {
            obj.insert("endpoint_used".into(), json!(endpoint));
            obj.insert("http_method".into(), json!("POST"));
            obj.insert("location".into(), json!(location));
            obj.insert("project_id_present".into(), json!(!project_id.is_empty()));
            obj.insert(
                "access_token_present".into(),
                json!(!access_token.is_empty()),
            );
            obj.insert(
                "access_token_masked_preview".into(),
                json!(mask_secret(&access_token)),
            );
            obj.insert(
                "model_requested".into(),
                json!(if model.is_empty() {
                    Value::Null
                } else {
                    json!(model)
                }),
            );
            obj.insert(
                "status_message".into(),
                json!("Cloud Translation - Advanced (v3) via REST. Requires OAuth access token; API keys are not supported."),
            );
        }
        diag
    }
}
