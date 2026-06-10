use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{json, Value};

use std::sync::Arc;

use super::{
    http::SharedHttpClient,
    base_diagnostics, http, normalize_source_lang, ProviderError, ProviderInfo, TranslateRequest,
    TranslationProvider,
};

const DEFAULT_SUBTITLE_TRANSLATION_PROMPT: &str =
    "You are a subtitle translator for livestream captions. \
Translate only the user subtitle text into the requested target language. \
Output only the translated subtitle text. \
Do not explain anything. Do not add notes, prefixes, quotes, brackets, or assistant-style chatter. \
Do not repeat the source text. Do not include the target language name. \
Keep the output concise, readable, and subtitle-friendly. \
Preserve names, game terms, UI labels, and obvious proper nouns when appropriate.";

const LLM_BASE_MAX_TOKENS: u32 = 96;
const LLM_TOKENS_PER_INPUT_CHAR: u32 = 6;
const LLM_MAX_TOKENS_CAP: u32 = 1024;

fn estimate_llm_max_tokens(text: &str) -> u32 {
    let estimated = LLM_BASE_MAX_TOKENS + text.chars().count() as u32 * LLM_TOKENS_PER_INPUT_CHAR;
    estimated.clamp(LLM_BASE_MAX_TOKENS, LLM_MAX_TOKENS_CAP)
}

pub struct OpenAICompatibleChatProvider {
    transport: Arc<SharedHttpClient>,
    name: &'static str,
    group: &'static str,
    default_base_url: &'static str,
    requires_api_key: bool,
    local_provider: bool,
}

impl OpenAICompatibleChatProvider {
    pub fn new(
        transport: Arc<SharedHttpClient>,
        name: &'static str,
        group: &'static str,
        default_base_url: &'static str,
        requires_api_key: bool,
        local_provider: bool,
    ) -> Self {
        Self {
            transport,
            name,
            group,
            default_base_url,
            requires_api_key,
            local_provider,
        }
    }

    fn build_messages(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
        custom_prompt: &str,
    ) -> (Vec<Value>, bool) {
        let system_prompt = if custom_prompt.is_empty() {
            DEFAULT_SUBTITLE_TRANSLATION_PROMPT
        } else {
            custom_prompt
        };
        let used_default_prompt = custom_prompt.is_empty();
        let normalized_source = normalize_source_lang(source_lang);
        let user_prompt = if normalized_source == "auto" {
            format!(
                "Detect the source language and translate the subtitle text into '{target_lang}'. \
Return only the translated subtitle text.\n\nSubtitle text:\n{text}"
            )
        } else {
            format!(
                "Translate the subtitle text from '{normalized_source}' into '{target_lang}'. \
Return only the translated subtitle text.\n\nSubtitle text:\n{text}"
            )
        };

        let messages = vec![
            json!({ "role": "system", "content": system_prompt }),
            json!({ "role": "user", "content": user_prompt }),
        ];
        (messages, used_default_prompt)
    }
}

#[async_trait]
impl TranslationProvider for OpenAICompatibleChatProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: self.name,
            group: self.group,
            experimental: false,
            local_provider: self.local_provider,
        }
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        let base_url = http::setting(request.settings, "base_url");
        let base_url = if base_url.is_empty() {
            self.default_base_url.to_string()
        } else {
            base_url
        };
        let api_key = http::setting(request.settings, "api_key");
        let model = http::setting(request.settings, "model");
        let custom_prompt = http::setting(request.settings, "custom_prompt");

        if self.requires_api_key && api_key.is_empty() {
            return Err(ProviderError::Message(format!(
                "{} API key is missing.",
                self.name
            )));
        }
        if base_url.is_empty() {
            return Err(ProviderError::Message(format!(
                "{} base URL is missing.",
                self.name
            )));
        }
        if model.is_empty() {
            return Err(ProviderError::Message(format!(
                "{} model is missing.",
                self.name
            )));
        }

        let (messages, _used_default_prompt) = self.build_messages(
            request.text,
            request.source_lang,
            request.target_lang,
            &custom_prompt,
        );
        let max_tokens = estimate_llm_max_tokens(request.text);
        let body = json!({
            "model": model,
            "messages": messages,
            "temperature": 0.2,
            "max_tokens": max_tokens,
        });

        let auth_header = if api_key.is_empty() {
            None
        } else {
            Some(format!("Bearer {api_key}"))
        };

        let mut header_pairs = vec![("Content-Type", "application/json")];
        if let Some(auth) = auth_header.as_deref() {
            header_pairs.push(("Authorization", auth));
        }

        let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
        let payload = http::request_json(
            &self.transport.client(),
            Method::POST,
            &url,
            None,
            Some(&body),
            None,
            Some(&header_pairs),
            &format!("{} request failed", self.name),
        )
        .await?;

        let translated = payload
            .get("choices")
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .and_then(|item| item.get("message"))
            .and_then(|message| message.get("content"))
            .map(extract_message_content)
            .unwrap_or_default();

        if translated.is_empty() {
            return Err(ProviderError::Message(format!(
                "{} returned an empty translation.",
                self.name
            )));
        }
        Ok(translated)
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        let custom_prompt = http::setting(settings, "custom_prompt");
        let base_url = http::setting(settings, "base_url");
        let base_url = if base_url.is_empty() {
            self.default_base_url.to_string()
        } else {
            base_url
        };
        let model = http::setting(settings, "model");

        let mut diag = base_diagnostics(&self.info(), settings);
        if let Some(obj) = diag.as_object_mut() {
            obj.insert("provider_endpoint".into(), json!(base_url));
            obj.insert(
                "model".into(),
                json!(if model.is_empty() {
                    Value::Null
                } else {
                    json!(model)
                }),
            );
            obj.insert(
                "used_default_prompt".into(),
                json!(custom_prompt.is_empty()),
            );
        }
        diag
    }
}

fn extract_message_content(content: &Value) -> String {
    match content {
        Value::String(text) => text.trim().to_string(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| {
                part.as_object()
                    .and_then(|obj| obj.get("text"))
                    .and_then(|value| value.as_str())
            })
            .collect::<String>()
            .trim()
            .to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn estimate_llm_max_tokens_scales_with_input() {
        assert_eq!(estimate_llm_max_tokens(""), 96);
        assert_eq!(estimate_llm_max_tokens(&"a".repeat(20)), 96 + 20 * 6);
        assert_eq!(estimate_llm_max_tokens(&"a".repeat(200)), 1024);
    }

    #[test]
    fn extract_message_content_handles_string_and_parts() {
        assert_eq!(extract_message_content(&json!("  hello  ")), "hello");
        assert_eq!(
            extract_message_content(&json!([{ "text": "hi" }, { "text": " there" }])),
            "hi there"
        );
    }
}
