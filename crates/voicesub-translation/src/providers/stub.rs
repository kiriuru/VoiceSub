use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::time::sleep;

use std::sync::Arc;

use super::{
    ProviderError, ProviderInfo, TranslateRequest, TranslationProvider, base_diagnostics, http,
    http::SharedHttpClient,
};
use serde_json::json;

/// Deterministic provider for dispatcher integration tests (SST `_StubTranslationEngine`).
pub struct StubTranslationProvider {
    transport: Arc<SharedHttpClient>,
}

impl StubTranslationProvider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self { transport }
    }

    fn delay_ms(settings: &HashMap<String, String>) -> u64 {
        let slot_id = settings.get("__slot_id").map(String::as_str).unwrap_or("");
        let slot_delay = format!("delay_ms_{slot_id}");
        if let Some(value) = settings
            .get(&slot_delay)
            .or_else(|| settings.get("delay_ms"))
        {
            return value.parse().unwrap_or(10);
        }
        10
    }
}

#[async_trait]
impl TranslationProvider for StubTranslationProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "stub",
            group: "stable",
            experimental: false,
            local_provider: false,
        }
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        let settings = request.settings;
        let slot_id = settings.get("__slot_id").map(String::as_str).unwrap_or("");
        let fail_slot = http::setting(settings, "fail_slot");
        if !fail_slot.is_empty() && (fail_slot == slot_id || fail_slot == request.target_lang) {
            return Err(ProviderError::Message(format!("{fail_slot} exploded")));
        }
        let delay_ms = Self::delay_ms(settings);
        sleep(Duration::from_millis(delay_ms)).await;
        Ok(format!("{}-{}", request.text.trim(), request.target_lang))
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        let mut diagnostics = base_diagnostics(&self.info(), settings);
        if let Some(obj) = diagnostics.as_object_mut() {
            obj.insert("http_client_bound".into(), json!(self.transport.is_bound()));
        }
        diagnostics
    }
}
