use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Value, json};

use std::sync::Arc;

use super::{
    ProviderError, ProviderInfo, TranslateRequest, TranslationProvider, base_diagnostics,
    crypto_util::sha256_hex, http, http::SharedHttpClient, lang_codes::youdao_lang, mask_secret,
    normalize_source_lang,
};

pub struct YoudaoTranslateProvider {
    transport: Arc<SharedHttpClient>,
}

impl YoudaoTranslateProvider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self { transport }
    }

    fn input_for_sign(q: &str) -> String {
        let chars: Vec<char> = q.chars().collect();
        if chars.len() <= 20 {
            q.to_string()
        } else {
            let head: String = chars.iter().take(10).collect();
            let tail: String = chars
                .iter()
                .rev()
                .take(10)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            format!("{head}{}{tail}", chars.len())
        }
    }
}

#[async_trait]
impl TranslationProvider for YoudaoTranslateProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "youdao_translate",
            group: "china",
            experimental: false,
            local_provider: false,
        }
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        let app_key = http::setting(request.settings, "app_key");
        let app_secret = http::setting(request.settings, "app_secret");
        if app_key.is_empty() || app_secret.is_empty() {
            return Err(ProviderError::Message(
                "Youdao Translate app_key and app_secret are required.".into(),
            ));
        }

        let source = normalize_source_lang(request.source_lang);
        let from = if source == "auto" {
            "auto".to_string()
        } else {
            youdao_lang(&source)
        };
        let to = youdao_lang(request.target_lang);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let salt = format!("{}{}", now.as_nanos(), request.text.len());
        let curtime = now.as_secs().to_string();
        let input = Self::input_for_sign(request.text);
        let sign = sha256_hex(&format!("{app_key}{input}{salt}{curtime}{app_secret}"));

        let form = [
            ("q", request.text),
            ("from", from.as_str()),
            ("to", to.as_str()),
            ("appKey", app_key.as_str()),
            ("salt", salt.as_str()),
            ("sign", sign.as_str()),
            ("signType", "v3"),
            ("curtime", curtime.as_str()),
        ];

        let payload = http::request_json(
            &self.transport.client(),
            Method::POST,
            "https://openapi.youdao.com/api",
            None,
            None,
            Some(&form),
            None,
            "Youdao Translate request failed",
            request.timeout_secs,
        )
        .await?;

        let error_code = payload
            .get("errorCode")
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .or_else(|| v.as_u64().map(|n| n.to_string()))
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
                    .unwrap_or_else(|| v.to_string())
            })
            .unwrap_or_else(|| "0".into());
        if error_code != "0" {
            return Err(ProviderError::Message(format!(
                "Youdao Translate errorCode={error_code}"
            )));
        }

        let translated = payload
            .get("translation")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
            .trim()
            .to_string();

        if translated.is_empty() {
            return Err(ProviderError::Message(
                "Youdao Translate returned an empty translation.".into(),
            ));
        }
        Ok(translated)
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        let app_key = http::setting(settings, "app_key");
        let app_secret = http::setting(settings, "app_secret");
        let mut diag = base_diagnostics(&self.info(), settings);
        if let Some(obj) = diag.as_object_mut() {
            obj.insert("app_key_present".into(), json!(!app_key.is_empty()));
            obj.insert(
                "app_secret_masked_preview".into(),
                json!(mask_secret(&app_secret)),
            );
            obj.insert(
                "status_message".into(),
                json!("Youdao Zhiyun text translate API (ai.youdao.com). Free trial quota after app creation."),
            );
        }
        diag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_for_sign_short_and_long() {
        assert_eq!(YoudaoTranslateProvider::input_for_sign("hello"), "hello");
        let long = "abcdefghijklmnopqrstuvwxyz"; // 26 chars
        assert_eq!(
            YoudaoTranslateProvider::input_for_sign(long),
            "abcdefghij26qrstuvwxyz"
        );
    }
}
