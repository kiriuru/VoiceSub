use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Value, json};

use std::sync::Arc;

use super::{
    ProviderError, ProviderInfo, TranslateRequest, TranslationProvider, base_diagnostics, http,
    http::SharedHttpClient,
    lang_codes::caiyun_lang,
    mask_secret, normalize_source_lang,
};

pub struct CaiyunTranslatorProvider {
    transport: Arc<SharedHttpClient>,
}

impl CaiyunTranslatorProvider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self { transport }
    }
}

#[async_trait]
impl TranslationProvider for CaiyunTranslatorProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "caiyun_translator",
            group: "china",
            experimental: false,
            local_provider: false,
        }
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        let token = http::setting(request.settings, "token");
        if token.is_empty() {
            return Err(ProviderError::Message(
                "Caiyun Translator token is missing.".into(),
            ));
        }

        let source = normalize_source_lang(request.source_lang);
        let from = if source == "auto" {
            "auto".to_string()
        } else {
            caiyun_lang(&source).map_err(ProviderError::Message)?
        };
        let to = caiyun_lang(request.target_lang).map_err(ProviderError::Message)?;
        let trans_type = format!("{from}2{to}");

        let body = json!({
            "source": request.text,
            "trans_type": trans_type,
            "request_id": "voicesub",
            "detect": from == "auto",
        });
        let auth = format!("token {token}");
        let headers = [("x-authorization", auth.as_str())];

        let payload = http::request_json(
            &self.transport.client(),
            Method::POST,
            "https://api.interpreter.caiyunai.com/v1/translator",
            None,
            Some(&body),
            None,
            Some(&headers),
            "Caiyun Translator request failed",
            request.timeout_secs,
        )
        .await?;

        let translated = payload
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if translated.is_empty() {
            let detail = payload
                .get("message")
                .or_else(|| payload.get("msg"))
                .and_then(|v| v.as_str())
                .unwrap_or("empty translation");
            return Err(ProviderError::Message(format!(
                "Caiyun Translator failed: {detail}"
            )));
        }
        Ok(translated)
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        let token = http::setting(settings, "token");
        let mut diag = base_diagnostics(&self.info(), settings);
        if let Some(obj) = diag.as_object_mut() {
            obj.insert("token_present".into(), json!(!token.is_empty()));
            obj.insert(
                "token_masked_preview".into(),
                json!(mask_secret(&token)),
            );
            obj.insert(
                "status_message".into(),
                json!("Caiyun Xiaoyi (彩云小译). Supports zh/en/ja. Token from https://fanyi.caiyunapp.com/."),
            );
        }
        diag
    }
}
