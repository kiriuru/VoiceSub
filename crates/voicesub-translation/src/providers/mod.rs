mod azure;
mod baidu;
mod caiyun;
mod crypto_util;
mod deepl;
mod experimental_google_web;
mod google_gas;
mod google_v2;
mod google_v3;
mod http;
mod lang_codes;
mod libretranslate;
mod openai_compatible;
mod public_mirrors;
mod stub;
mod tencent_tmt;
mod youdao;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};
use thiserror::Error;

pub use azure::AzureTranslatorProvider;
pub use baidu::BaiduTranslateProvider;
pub use caiyun::CaiyunTranslatorProvider;
pub use deepl::{DeepLProvider, resolve_deepl_api_url};
pub use experimental_google_web::{FreeWebTranslateProvider, GoogleWebProvider};
pub use google_gas::GoogleGasUrlProvider;
pub use google_v2::GoogleTranslateV2Provider;
pub use google_v3::GoogleCloudTranslationV3Provider;
pub use http::{
    MAX_HTTP_REQUEST_TIMEOUT_SECONDS, SharedHttpClient, build_translation_http_client,
    effective_request_timeout,
};
pub use libretranslate::LibreTranslateProvider;
pub use openai_compatible::OpenAICompatibleChatProvider;
pub use public_mirrors::PublicLibreTranslateMirrorProvider;
pub use stub::StubTranslationProvider;
pub use tencent_tmt::TencentTmtProvider;
pub use youdao::YoudaoTranslateProvider;

pub const SUPPORTED_PROVIDERS: &[&str] = &[
    "google_translate_v2",
    "google_cloud_translation_v3",
    "google_gas_url",
    "google_web",
    "azure_translator",
    "deepl",
    "libretranslate",
    "openai",
    "openrouter",
    "lm_studio",
    "ollama",
    "public_libretranslate_mirror",
    "free_web_translate",
    "baidu_translate",
    "youdao_translate",
    "tencent_tmt",
    "caiyun_translator",
];

#[derive(Debug, Clone, Copy)]
pub struct ProviderInfo {
    pub name: &'static str,
    pub group: &'static str,
    pub experimental: bool,
    pub local_provider: bool,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("{0}")]
    Message(String),
    #[error("retryable: {0}")]
    Retryable(String),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
}

impl ProviderError {
    pub fn retryable(message: impl Into<String>) -> Self {
        Self::Retryable(message.into())
    }

    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Retryable(_) => true,
            Self::Http(err) => err.is_timeout() || err.is_connect() || err.is_request(),
            Self::Message(message) => {
                let lower = message.to_ascii_lowercase();
                lower.contains("http 5")
                    || lower.contains("http 429")
                    || lower.contains("timed out")
                    || lower.contains("network error")
                    // LM Studio JIT: first attempt may abort while the engine starts.
                    || lower.contains("model is unloaded")
                    || lower.contains("channel error")
                    || lower.contains("startup was aborted")
            }
        }
    }
}

pub const DEFAULT_REQUEST_TIMEOUT_SECONDS: f64 = 20.0;

/// SST `translation_engine.py` — `_DEFAULT_HTTP_KEEPALIVE_LIMIT`.
pub const DEFAULT_HTTP_KEEPALIVE_LIMIT: usize = 20;
/// SST `translation_engine.py` — `_DEFAULT_HTTP_KEEPALIVE_EXPIRY_SECONDS`.
pub const DEFAULT_HTTP_KEEPALIVE_EXPIRY_SECONDS: u64 = 30;
/// SST httpx client — connect timeout.
pub const DEFAULT_HTTP_CONNECT_TIMEOUT_SECONDS: u64 = 10;

pub struct TranslateRequest<'a> {
    pub text: &'a str,
    pub source_lang: &'a str,
    pub target_lang: &'a str,
    pub settings: &'a HashMap<String, String>,
    pub timeout_secs: Option<f64>,
}

#[async_trait]
pub trait TranslationProvider: Send + Sync {
    fn info(&self) -> ProviderInfo;
    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError>;
    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value;
}

pub fn build_default_registry(
    transport: Arc<SharedHttpClient>,
) -> HashMap<String, Arc<dyn TranslationProvider>> {
    let mut registry: HashMap<String, Arc<dyn TranslationProvider>> = HashMap::new();
    registry.insert(
        "google_translate_v2".into(),
        Arc::new(GoogleTranslateV2Provider::new(transport.clone())),
    );
    registry.insert(
        "google_cloud_translation_v3".into(),
        Arc::new(GoogleCloudTranslationV3Provider::new(transport.clone())),
    );
    registry.insert(
        "google_gas_url".into(),
        Arc::new(GoogleGasUrlProvider::new(transport.clone())),
    );
    registry.insert(
        "google_web".into(),
        Arc::new(GoogleWebProvider::new(transport.clone())),
    );
    registry.insert(
        "azure_translator".into(),
        Arc::new(AzureTranslatorProvider::new(transport.clone())),
    );
    registry.insert(
        "deepl".into(),
        Arc::new(DeepLProvider::new(transport.clone())),
    );
    registry.insert(
        "libretranslate".into(),
        Arc::new(LibreTranslateProvider::new(transport.clone())),
    );
    registry.insert(
        "openai".into(),
        Arc::new(OpenAICompatibleChatProvider::new(
            transport.clone(),
            "openai",
            "llm",
            "https://api.openai.com/v1",
            true,
            false,
        )),
    );
    registry.insert(
        "openrouter".into(),
        Arc::new(OpenAICompatibleChatProvider::new(
            transport.clone(),
            "openrouter",
            "llm",
            "https://openrouter.ai/api/v1",
            true,
            false,
        )),
    );
    registry.insert(
        "lm_studio".into(),
        Arc::new(OpenAICompatibleChatProvider::new(
            transport.clone(),
            "lm_studio",
            "local_llm",
            "http://127.0.0.1:1234/v1",
            false,
            true,
        )),
    );
    registry.insert(
        "ollama".into(),
        Arc::new(OpenAICompatibleChatProvider::new(
            transport.clone(),
            "ollama",
            "local_llm",
            "http://127.0.0.1:11434/v1",
            false,
            true,
        )),
    );
    registry.insert(
        "public_libretranslate_mirror".into(),
        Arc::new(PublicLibreTranslateMirrorProvider::new(transport.clone())),
    );
    registry.insert(
        "free_web_translate".into(),
        Arc::new(FreeWebTranslateProvider::new(transport.clone())),
    );
    registry.insert(
        "baidu_translate".into(),
        Arc::new(BaiduTranslateProvider::new(transport.clone())),
    );
    registry.insert(
        "youdao_translate".into(),
        Arc::new(YoudaoTranslateProvider::new(transport.clone())),
    );
    registry.insert(
        "tencent_tmt".into(),
        Arc::new(TencentTmtProvider::new(transport.clone())),
    );
    registry.insert(
        "caiyun_translator".into(),
        Arc::new(CaiyunTranslatorProvider::new(transport)),
    );
    registry
}

pub fn canonical_provider_name(raw: &str) -> String {
    let name = raw.trim();
    if name == "stub" || SUPPORTED_PROVIDERS.contains(&name) {
        name.to_string()
    } else {
        "google_translate_v2".into()
    }
}

pub fn normalize_source_lang(source_lang: &str) -> String {
    let trimmed = source_lang.trim().to_ascii_lowercase();
    if trimmed.is_empty() || trimmed == "auto" {
        "auto".into()
    } else {
        trimmed
    }
}

pub fn mask_secret(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 4 {
        return "****".into();
    }
    format!("{}****", &trimmed[..4.min(trimmed.len())])
}

pub fn base_diagnostics(info: &ProviderInfo, settings: &HashMap<String, String>) -> Value {
    json!({
        "provider": info.name,
        "provider_group": info.group,
        "experimental": info.experimental,
        "local_provider": info.local_provider,
        "settings_keys": settings.keys().collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_registers_all_supported_providers() {
        let transport = SharedHttpClient::new(reqwest::Client::new());
        let registry = build_default_registry(transport);
        for name in SUPPORTED_PROVIDERS {
            assert!(
                registry.contains_key(*name),
                "missing provider registration for {name}"
            );
        }
        assert_eq!(registry.len(), SUPPORTED_PROVIDERS.len());
    }

    #[test]
    fn openai_provider_requires_api_key() {
        let transport = SharedHttpClient::new(reqwest::Client::new());
        let registry = build_default_registry(transport);
        let provider = registry.get("openai").expect("openai provider");
        let settings = HashMap::new();
        let request = TranslateRequest {
            text: "hello",
            source_lang: "en",
            target_lang: "ru",
            settings: &settings,
            timeout_secs: None,
        };
        let err = tokio::runtime::Runtime::new()
            .expect("runtime")
            .block_on(provider.translate(request))
            .expect_err("expected missing api key error");
        assert!(err.to_string().contains("API key is missing"));
    }
}
