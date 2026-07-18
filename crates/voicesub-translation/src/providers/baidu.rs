use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Value, json};

use std::sync::Arc;

use super::{
    ProviderError, ProviderInfo, TranslateRequest, TranslationProvider, base_diagnostics,
    crypto_util::md5_hex, http, http::SharedHttpClient,
    lang_codes::baidu_lang,
    mask_secret, normalize_source_lang,
};

pub struct BaiduTranslateProvider {
    transport: Arc<SharedHttpClient>,
}

impl BaiduTranslateProvider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self { transport }
    }
}

#[async_trait]
impl TranslationProvider for BaiduTranslateProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "baidu_translate",
            group: "china",
            experimental: false,
            local_provider: false,
        }
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        let app_id = http::setting(request.settings, "app_id");
        let secret_key = http::setting(request.settings, "secret_key");
        if app_id.is_empty() || secret_key.is_empty() {
            return Err(ProviderError::Message(
                "Baidu Translate app_id and secret_key are required.".into(),
            ));
        }

        let source = normalize_source_lang(request.source_lang);
        let from = if source == "auto" {
            "auto".to_string()
        } else {
            baidu_lang(&source)
        };
        let to = baidu_lang(request.target_lang);
        let salt = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis().to_string())
            .unwrap_or_else(|_| "1".into());
        let sign = md5_hex(&format!("{app_id}{}{salt}{secret_key}", request.text));

        // POST + form-urlencoded: Baidu allows GET or POST; POST avoids URL-length
        // limits on longer subtitle lines and matches the recommended client pattern.
        let form = [
            ("q", request.text),
            ("from", from.as_str()),
            ("to", to.as_str()),
            ("appid", app_id.as_str()),
            ("salt", salt.as_str()),
            ("sign", sign.as_str()),
        ];

        let payload = http::request_json(
            &self.transport.client(),
            Method::POST,
            "https://fanyi-api.baidu.com/api/trans/vip/translate",
            None,
            None,
            Some(&form),
            None,
            "Baidu Translate request failed",
            request.timeout_secs,
        )
        .await?;

        if let Some(error_code) = payload.get("error_code") {
            let code = error_code
                .as_str()
                .map(str::to_string)
                .or_else(|| error_code.as_u64().map(|v| v.to_string()))
                .unwrap_or_else(|| error_code.to_string());
            if code != "0" {
                let msg = payload
                    .get("error_msg")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                return Err(ProviderError::Message(format!(
                    "Baidu Translate error {code}: {msg}"
                )));
            }
        }

        let translated = payload
            .get("trans_result")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("dst").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
            .trim()
            .to_string();

        if translated.is_empty() {
            return Err(ProviderError::Message(
                "Baidu Translate returned an empty translation.".into(),
            ));
        }
        Ok(translated)
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        let app_id = http::setting(settings, "app_id");
        let secret_key = http::setting(settings, "secret_key");
        let mut diag = base_diagnostics(&self.info(), settings);
        if let Some(obj) = diag.as_object_mut() {
            obj.insert("app_id_present".into(), json!(!app_id.is_empty()));
            obj.insert(
                "secret_key_masked_preview".into(),
                json!(mask_secret(&secret_key)),
            );
            obj.insert(
                "status_message".into(),
                json!("Baidu Translate (通用翻译 API). Free monthly quota available after app registration on fanyi-api.baidu.com."),
            );
        }
        diag
    }
}
