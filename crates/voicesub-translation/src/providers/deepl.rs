use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Value, json};

use std::sync::Arc;

use super::{
    ProviderError, ProviderInfo, TranslateRequest, TranslationProvider, base_diagnostics, http,
    http::SharedHttpClient,
    lang_codes::{deepl_source_lang, deepl_target_lang},
};

pub const DEEPL_FREE_API_URL: &str = "https://api-free.deepl.com/v2/translate";
pub const DEEPL_PRO_API_URL: &str = "https://api.deepl.com/v2/translate";

pub struct DeepLProvider {
    transport: Arc<SharedHttpClient>,
}

impl DeepLProvider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self { transport }
    }
}

/// Free DeepL keys end with `:fx`. Treat empty / stock free|pro URLs as auto-select.
pub fn resolve_deepl_api_url(api_key: &str, configured_api_url: &str) -> String {
    let free_key = api_key.contains(":fx");
    let auto = if free_key {
        DEEPL_FREE_API_URL
    } else {
        DEEPL_PRO_API_URL
    };
    let configured = configured_api_url.trim();
    if configured.is_empty()
        || configured.eq_ignore_ascii_case(DEEPL_FREE_API_URL)
        || configured.eq_ignore_ascii_case(DEEPL_PRO_API_URL)
    {
        return auto.to_string();
    }
    configured.to_string()
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
        let api_url = resolve_deepl_api_url(&api_key, &configured_api_url);

        let target_lang = deepl_target_lang(request.target_lang);
        let upper_source = deepl_source_lang(request.source_lang);
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
            request.timeout_secs,
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
        let api_key = http::setting(settings, "api_key");
        let configured = http::setting(settings, "api_url");
        let resolved = if api_key.is_empty() {
            if configured.trim().is_empty() {
                DEEPL_FREE_API_URL.to_string()
            } else {
                configured
            }
        } else {
            resolve_deepl_api_url(&api_key, &configured)
        };
        let mut diag = base_diagnostics(&self.info(), settings);
        if let Some(obj) = diag.as_object_mut() {
            obj.insert("endpoint_used".into(), json!(resolved));
            obj.insert(
                "deepl_api_tier".into(),
                json!(if api_key.contains(":fx") {
                    "free"
                } else if api_key.is_empty() {
                    "unknown"
                } else {
                    "pro"
                }),
            );
            obj.insert(
                "status_message".into(),
                json!(
                    "DeepL maps UI codes (en/zh-cn/pt) to API targets; Free vs Pro URL is chosen from the API key (:fx → free) unless a custom api_url is set."
                ),
            );
        }
        diag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_key_selects_free_url() {
        assert_eq!(
            resolve_deepl_api_url("abc:fx", ""),
            DEEPL_FREE_API_URL
        );
        assert_eq!(
            resolve_deepl_api_url("abc:fx", DEEPL_PRO_API_URL),
            DEEPL_FREE_API_URL
        );
    }

    #[test]
    fn pro_key_selects_pro_url_even_when_default_free_configured() {
        assert_eq!(
            resolve_deepl_api_url("pro-key-without-fx", DEEPL_FREE_API_URL),
            DEEPL_PRO_API_URL
        );
    }

    #[test]
    fn custom_url_is_preserved() {
        assert_eq!(
            resolve_deepl_api_url("pro-key", "https://proxy.example/translate"),
            "https://proxy.example/translate"
        );
    }
}
