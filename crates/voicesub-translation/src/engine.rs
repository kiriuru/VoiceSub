use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex as AsyncMutex;

use serde_json::Value;
use tokio::time::sleep;
use tracing::instrument;
use voicesub_subtitle::TranslationItem;

use crate::cache::{DEFAULT_MAX_ENTRIES, TranslationCache, cache_key};
use crate::providers::{
    DEFAULT_REQUEST_TIMEOUT_SECONDS, SharedHttpClient, StubTranslationProvider,
    TranslationProvider, build_default_registry, canonical_provider_name, normalize_source_lang,
};

const DEFAULT_PROVIDER: &str = "google_translate_v2";
const CANONICAL_SLOTS: [&str; 5] = [
    "translation_1",
    "translation_2",
    "translation_3",
    "translation_4",
    "translation_5",
];
const RETRY_BACKOFF_BASE_SECONDS: f64 = 0.3;
const RETRY_BACKOFF_MAX_SECONDS: f64 = 2.0;
pub const DEFAULT_TRANSLATION_RETRIES: u32 = 2;

struct CacheSettings {
    enabled: bool,
    persist: bool,
    max_entries: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct NormalizedLine {
    pub slot_id: String,
    pub enabled: bool,
    pub target_lang: String,
    pub provider: String,
    pub label: String,
}

#[derive(Debug, Clone, Default)]
pub struct TranslateTargetOptions {
    pub slot_id: Option<String>,
    pub label: Option<String>,
    pub budget_seconds: Option<f64>,
    pub retries: u32,
}

#[derive(Debug, Clone)]
struct FailedTranslationItem {
    target_lang: String,
    provider_name: String,
    provider_group: String,
    experimental: bool,
    local_provider: bool,
    slot_id: Option<String>,
    label: Option<String>,
    message: String,
}

struct TranslateRetryRequest<'a> {
    provider: Arc<dyn TranslationProvider>,
    source_text: &'a str,
    source_lang: &'a str,
    target_lang: &'a str,
    provider_settings: &'a HashMap<String, String>,
    retries: u32,
    slot_id: Option<String>,
    label: Option<String>,
    line: &'a PreparedLine,
    budget_seconds: Option<f64>,
}

struct LineFetchWork {
    provider: Arc<dyn TranslationProvider>,
    source_text: String,
    normalized_source: String,
    normalized_target: String,
    call_settings: HashMap<String, String>,
    retries: u32,
    slot_id: Option<String>,
    label: Option<String>,
    line: PreparedLine,
    budget_seconds: Option<f64>,
    cache_key: String,
}

enum LineTranslatePlan {
    Done(TranslationItem, Value),
    Fetch(LineFetchWork),
}

#[derive(Debug, Clone)]
pub struct PreparedLine {
    pub slot_id: String,
    pub target_lang: String,
    pub provider_name: String,
    pub provider_settings: HashMap<String, String>,
    pub provider_group: String,
    pub experimental: bool,
    pub local_provider: bool,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct PreparedRequest {
    pub provider_name: String,
    pub target_languages: Vec<String>,
    pub provider_group: String,
    pub experimental: bool,
    pub local_provider: bool,
    pub lines: Vec<PreparedLine>,
}

#[derive(Debug, Clone)]
pub struct TranslationBatch {
    pub provider: String,
    pub source_lang: String,
    pub target_languages: Vec<String>,
    pub items: Vec<TranslationItem>,
    pub provider_group: String,
    pub experimental: bool,
    pub local_provider: bool,
    pub used_default_prompt: bool,
    pub status_message: Option<String>,
}

pub struct TranslationEngine {
    providers: HashMap<String, Arc<dyn TranslationProvider>>,
    cache: Arc<TranslationCache>,
    settings_signature: Option<String>,
    http_transport: Arc<SharedHttpClient>,
}

impl TranslationEngine {
    pub fn new(client: reqwest::Client, cache_dir: Option<PathBuf>) -> Self {
        let transport = SharedHttpClient::new(client.clone());
        transport.bind(Arc::new({
            let client = client;
            move || client.clone()
        }));
        let providers = build_default_registry(transport.clone());
        Self {
            providers,
            cache: Arc::new(TranslationCache::with_dir(cache_dir, DEFAULT_MAX_ENTRIES)),
            settings_signature: None,
            http_transport: transport,
        }
    }

    pub fn with_providers(
        providers: HashMap<String, Arc<dyn TranslationProvider>>,
        cache_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            providers,
            cache: Arc::new(TranslationCache::with_dir(cache_dir, DEFAULT_MAX_ENTRIES)),
            settings_signature: None,
            http_transport: SharedHttpClient::new(crate::providers::build_translation_http_client()),
        }
    }

    pub fn new_stub(client: reqwest::Client) -> Self {
        let transport = SharedHttpClient::new(client);
        let mut providers = build_default_registry(transport.clone());
        providers.insert(
            "stub".into(),
            Arc::new(StubTranslationProvider::new(transport)),
        );
        Self::with_providers(providers, None)
    }

    pub fn http_transport(&self) -> &Arc<SharedHttpClient> {
        &self.http_transport
    }

    pub fn provider(&self, name: &str) -> Option<Arc<dyn TranslationProvider>> {
        self.providers.get(name).cloned()
    }

    pub fn summarize_readiness(&self, translation_config: &Value) -> Value {
        crate::readiness::summarize_readiness(
            &self.providers,
            translation_config,
            &Self::normalized_lines(translation_config),
            &Self::normalized_provider_settings(translation_config),
        )
    }

    #[doc(hidden)]
    pub fn seed_translation_cache(
        &self,
        provider_name: &str,
        source_lang: &str,
        target_lang: &str,
        source_text: &str,
        translated: &str,
    ) {
        let key = cache_key(
            provider_name,
            &normalize_source_lang(source_lang),
            &target_lang.trim().to_ascii_lowercase(),
            source_text,
        );
        self.cache.insert(key, translated.to_string());
    }

    pub fn apply_live_settings(&mut self, translation_config: &Value) {
        let signature = Self::build_settings_signature(translation_config);
        // First apply only records the signature — do not wipe a persisted cache on startup.
        if let Some(previous) = self.settings_signature.as_deref()
            && previous != signature.as_str()
        {
            self.cache.clear();
        }
        self.settings_signature = Some(signature);
        let cache_cfg = Self::normalized_cache_settings(translation_config);
        self.cache
            .update_settings(cache_cfg.enabled, cache_cfg.persist, cache_cfg.max_entries);
    }

    pub fn prepare_request(&self, translation_config: &Value) -> PreparedRequest {
        let provider_settings_map = Self::normalized_provider_settings(translation_config);
        let mut lines = Vec::new();

        for line in Self::normalized_lines(translation_config) {
            if !line.enabled {
                continue;
            }
            let provider_name = canonical_provider_name(&line.provider);
            let provider = self.providers.get(&provider_name);
            let provider_group = provider
                .map(|p| p.info().group.to_string())
                .unwrap_or_else(|| "experimental".into());
            let experimental = provider.map(|p| p.info().experimental).unwrap_or(true);
            let local_provider = provider.map(|p| p.info().local_provider).unwrap_or(false);
            let provider_settings = provider_settings_map
                .get(&provider_name)
                .cloned()
                .unwrap_or_default();
            lines.push(PreparedLine {
                slot_id: line.slot_id,
                target_lang: line.target_lang,
                provider_name,
                provider_settings,
                provider_group,
                experimental,
                local_provider,
                label: line.label,
            });
        }

        let provider_names: Vec<_> = lines.iter().map(|l| l.provider_name.as_str()).collect();
        let provider_groups: Vec<_> = lines.iter().map(|l| l.provider_group.as_str()).collect();
        let provider_name = if provider_names.is_empty() {
            canonical_provider_name(
                translation_config
                    .get("provider")
                    .and_then(|v| v.as_str())
                    .unwrap_or(DEFAULT_PROVIDER),
            )
        } else if provider_names.windows(2).all(|w| w[0] == w[1]) {
            provider_names[0].to_string()
        } else {
            "mixed".into()
        };
        let provider_group = if provider_groups.is_empty() {
            "experimental".into()
        } else if provider_groups.windows(2).all(|w| w[0] == w[1]) {
            provider_groups[0].to_string()
        } else {
            "mixed".into()
        };

        PreparedRequest {
            provider_name,
            target_languages: lines.iter().map(|l| l.target_lang.clone()).collect(),
            provider_group,
            experimental: lines.iter().any(|l| l.experimental),
            local_provider: lines.iter().any(|l| l.local_provider),
            lines,
        }
    }

    #[instrument(skip(self))]
    pub async fn translate_target(
        &mut self,
        source_text: &str,
        source_lang: &str,
        line: &PreparedLine,
        options: TranslateTargetOptions,
    ) -> (TranslationItem, Value) {
        match self.plan_line_translate(source_text, source_lang, line, &options) {
            LineTranslatePlan::Done(item, diagnostics) => (item, diagnostics),
            LineTranslatePlan::Fetch(work) => {
                let cache_key = work.cache_key.clone();
                let cache = Arc::clone(&self.cache);
                let (item, diagnostics) = Self::run_line_fetch(work).await;
                Self::cache_translation_result(&cache, &cache_key, &item);
                (item, diagnostics)
            }
        }
    }

    /// Like [`translate_target`] but releases the engine mutex before provider I/O so
    /// parallel translation lines are not serialized on the global engine lock.
    pub async fn translate_target_concurrent(
        engine: Arc<AsyncMutex<Self>>,
        source_text: &str,
        source_lang: &str,
        line: &PreparedLine,
        options: TranslateTargetOptions,
    ) -> (TranslationItem, Value) {
        let (plan, cache) = {
            let mut guard = engine.lock().await;
            let plan = guard.plan_line_translate(source_text, source_lang, line, &options);
            (plan, Arc::clone(&guard.cache))
        };
        match plan {
            LineTranslatePlan::Done(item, diagnostics) => (item, diagnostics),
            LineTranslatePlan::Fetch(work) => {
                let cache_key = work.cache_key.clone();
                let (item, diagnostics) = Self::run_line_fetch(work).await;
                Self::cache_translation_result(&cache, &cache_key, &item);
                (item, diagnostics)
            }
        }
    }

    fn plan_line_translate(
        &mut self,
        source_text: &str,
        source_lang: &str,
        line: &PreparedLine,
        options: &TranslateTargetOptions,
    ) -> LineTranslatePlan {
        let normalized_target = line.target_lang.trim().to_ascii_lowercase();
        let normalized_source = normalize_source_lang(source_lang);
        let slot = options
            .slot_id
            .clone()
            .or_else(|| Some(line.slot_id.clone()));
        let item_label = options.label.clone().or_else(|| Some(line.label.clone()));

        let provider = if let Some(provider) = self.providers.get(&line.provider_name) { provider.clone() } else {
            let message = format!("Unsupported translation provider: {}", line.provider_name);
            return LineTranslatePlan::Done(
                Self::failed_item(FailedTranslationItem {
                    target_lang: normalized_target,
                    provider_name: line.provider_name.clone(),
                    provider_group: line.provider_group.clone(),
                    experimental: line.experimental,
                    local_provider: line.local_provider,
                    slot_id: slot,
                    label: item_label,
                    message: message.clone(),
                }),
                serde_json::json!({ "status_message": message }),
            );
        };

        if source_text.trim().is_empty()
            || (normalized_source != "auto" && normalized_source == normalized_target)
        {
            let diagnostics = provider.diagnostics(&line.provider_settings);
            let mut diag = diagnostics;
            if let Some(obj) = diag.as_object_mut() {
                obj.insert(
                    "status_message".into(),
                    Value::String(
                        "Translation short-circuited (empty or identical source/target language)."
                            .into(),
                    ),
                );
            }
            return LineTranslatePlan::Done(
                TranslationItem {
                    target_lang: normalized_target,
                    text: source_text.to_string(),
                    provider: line.provider_name.clone(),
                    slot_id: slot,
                    label: item_label,
                    provider_group: Some(line.provider_group.clone()),
                    experimental: line.experimental,
                    local_provider: line.local_provider,
                    success: true,
                    error: None,
                    cached: false,
                },
                diag,
            );
        }

        let key = cache_key(
            &line.provider_name,
            &normalized_source,
            &normalized_target,
            source_text,
        );
        if self.cache.enabled()
            && let Some(cached) = self.cache.get(&key)
        {
            let diagnostics = provider.diagnostics(&line.provider_settings);
            return LineTranslatePlan::Done(
                TranslationItem {
                    target_lang: normalized_target,
                    text: cached,
                    provider: line.provider_name.clone(),
                    slot_id: slot,
                    label: item_label,
                    provider_group: Some(line.provider_group.clone()),
                    experimental: line.experimental,
                    local_provider: line.local_provider,
                    success: true,
                    error: None,
                    cached: true,
                },
                diagnostics,
            );
        }

        let mut call_settings = line.provider_settings.clone();
        call_settings.insert("__slot_id".into(), line.slot_id.clone());

        LineTranslatePlan::Fetch(LineFetchWork {
            provider,
            source_text: source_text.to_string(),
            normalized_source,
            normalized_target,
            call_settings,
            retries: options.retries,
            slot_id: slot,
            label: item_label,
            line: line.clone(),
            budget_seconds: options.budget_seconds,
            cache_key: key,
        })
    }

    async fn run_line_fetch(work: LineFetchWork) -> (TranslationItem, Value) {
        Self::translate_with_retry(TranslateRetryRequest {
            provider: work.provider,
            source_text: &work.source_text,
            source_lang: &work.normalized_source,
            target_lang: &work.normalized_target,
            provider_settings: &work.call_settings,
            retries: work.retries,
            slot_id: work.slot_id,
            label: work.label,
            line: &work.line,
            budget_seconds: work.budget_seconds,
        })
        .await
    }

    fn cache_translation_result(cache: &TranslationCache, cache_key: &str, item: &TranslationItem) {
        if item.success && !item.text.is_empty() && cache.enabled() {
            cache.insert(cache_key.to_string(), item.text.clone());
        }
    }

    pub async fn translate_targets(
        &self,
        source_text: &str,
        source_lang: &str,
        provider_name: &str,
        provider_settings: &HashMap<String, String>,
        target_languages: &[String],
        retries: u32,
    ) -> TranslationBatch {
        let provider_name = canonical_provider_name(provider_name);
        let clean_targets: Vec<String> = target_languages
            .iter()
            .map(|lang| lang.trim().to_ascii_lowercase())
            .filter(|lang| !lang.is_empty())
            .collect();
        let provider = match self.providers.get(&provider_name) {
            Some(provider) => provider.clone(),
            None => {
                return TranslationBatch {
                    provider: provider_name.clone(),
                    source_lang: source_lang.to_string(),
                    target_languages: clean_targets.clone(),
                    items: clean_targets
                        .iter()
                        .map(|target| {
                            Self::failed_item(FailedTranslationItem {
                                target_lang: target.clone(),
                                provider_name: provider_name.clone(),
                                provider_group: "experimental".into(),
                                experimental: true,
                                local_provider: false,
                                slot_id: None,
                                label: None,
                                message: "Unsupported translation provider".into(),
                            })
                        })
                        .collect(),
                    provider_group: "experimental".into(),
                    experimental: true,
                    local_provider: false,
                    used_default_prompt: false,
                    status_message: Some("Unsupported translation provider".into()),
                };
            }
        };

        let info = provider.info();
        let normalized_source = normalize_source_lang(source_lang);
        let mut used_default_prompt = false;
        let mut last_status = None;
        let mut items: Vec<Option<TranslationItem>> = vec![None; clean_targets.len()];
        let mut pending = Vec::new();

        for (index, target_lang) in clean_targets.iter().enumerate() {
            let key = cache_key(&provider_name, &normalized_source, target_lang, source_text);
            if self.cache.enabled()
                && let Some(cached) = self.cache.get(&key)
            {
                let diagnostics = provider.diagnostics(provider_settings);
                used_default_prompt |= diagnostics
                    .get("used_default_prompt")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                items[index] = Some(TranslationItem {
                    target_lang: target_lang.clone(),
                    text: cached,
                    provider: provider_name.clone(),
                    slot_id: None,
                    label: None,
                    provider_group: Some(info.group.to_string()),
                    experimental: info.experimental,
                    local_provider: info.local_provider,
                    success: true,
                    error: None,
                    cached: true,
                });
                continue;
            }

            let pseudo_line = PreparedLine {
                slot_id: String::new(),
                target_lang: target_lang.clone(),
                provider_name: provider_name.clone(),
                provider_settings: provider_settings.clone(),
                provider_group: info.group.to_string(),
                experimental: info.experimental,
                local_provider: info.local_provider,
                label: target_lang.to_ascii_uppercase(),
            };
            let provider = provider.clone();
            let source_text = source_text.to_string();
            let source_lang = normalized_source.clone();
            let target_lang = target_lang.clone();
            let provider_settings = provider_settings.clone();
            pending.push(tokio::spawn(async move {
                let (item, diagnostics) = Self::translate_with_retry(TranslateRetryRequest {
                    provider,
                    source_text: &source_text,
                    source_lang: &source_lang,
                    target_lang: &target_lang,
                    provider_settings: &provider_settings,
                    retries,
                    slot_id: None,
                    label: None,
                    line: &pseudo_line,
                    budget_seconds: None,
                })
                .await;
                (index, key, item, diagnostics)
            }));
        }

        let cache = Arc::clone(&self.cache);
        for handle in pending {
            let (index, key, item, diagnostics) = handle.await.expect("translate target task");
            used_default_prompt |= diagnostics
                .get("used_default_prompt")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            last_status = diagnostics
                .get("status_message")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or(last_status);
            if item.success && !item.text.is_empty() && cache.enabled() {
                cache.insert(key, item.text.clone());
            }
            items[index] = Some(item);
        }

        let items: Vec<TranslationItem> = items.into_iter().flatten().collect();

        TranslationBatch {
            provider: provider_name,
            source_lang: normalized_source,
            target_languages: clean_targets,
            items,
            provider_group: info.group.to_string(),
            experimental: info.experimental,
            local_provider: info.local_provider,
            used_default_prompt,
            status_message: last_status,
        }
    }

    async fn translate_with_retry(request: TranslateRetryRequest<'_>) -> (TranslationItem, Value) {
        let TranslateRetryRequest {
            provider,
            source_text,
            source_lang,
            target_lang,
            provider_settings,
            retries,
            slot_id,
            label,
            line,
            budget_seconds,
        } = request;
        let max_attempts = retries.saturating_add(1).max(1);
        let started_at = Instant::now();
        let normalized_budget = budget_seconds.filter(|budget| *budget > 0.0);
        let mut attempt = 0u32;
        let mut last_error = "Translation failed.".to_string();
        let mut last_diagnostics = provider.diagnostics(provider_settings);

        while attempt < max_attempts {
            attempt += 1;
            let per_attempt_timeout =
                Self::per_attempt_timeout(normalized_budget, started_at, max_attempts, attempt);
            let request = crate::providers::TranslateRequest {
                text: source_text,
                source_lang,
                target_lang,
                settings: provider_settings,
                timeout_secs: Some(per_attempt_timeout),
            };
            match tokio::time::timeout(
                Duration::from_secs_f64(per_attempt_timeout),
                provider.translate(request),
            )
            .await
            {
                Ok(Ok(text)) => {
                    let diagnostics = provider.diagnostics(provider_settings);
                    return (
                        TranslationItem {
                            target_lang: target_lang.to_string(),
                            text,
                            provider: line.provider_name.clone(),
                            slot_id,
                            label,
                            provider_group: Some(line.provider_group.clone()),
                            experimental: line.experimental,
                            local_provider: line.local_provider,
                            success: true,
                            error: None,
                            cached: false,
                        },
                        diagnostics,
                    );
                }
                Ok(Err(err)) => {
                    last_error = err.to_string();
                    last_diagnostics = provider.diagnostics(provider_settings);
                    if let Some(obj) = last_diagnostics.as_object_mut() {
                        obj.insert("status_message".into(), Value::String(last_error.clone()));
                    }
                    if attempt < max_attempts
                        && err.is_retryable()
                        && Self::has_remaining_budget(normalized_budget, started_at)
                    {
                        Self::sleep_with_jitter(attempt, normalized_budget, started_at).await;
                        continue;
                    }
                    break;
                }
                Err(_) => {
                    last_error = format!("Translation timed out after {per_attempt_timeout:.2}s.");
                    if let Some(obj) = last_diagnostics.as_object_mut() {
                        obj.insert("status_message".into(), Value::String(last_error.clone()));
                    }
                    if attempt < max_attempts
                        && Self::has_remaining_budget(normalized_budget, started_at)
                    {
                        Self::sleep_with_jitter(attempt, normalized_budget, started_at).await;
                        continue;
                    }
                    break;
                }
            }
        }

        (
            Self::failed_item(FailedTranslationItem {
                target_lang: target_lang.to_string(),
                provider_name: line.provider_name.clone(),
                provider_group: line.provider_group.clone(),
                experimental: line.experimental,
                local_provider: line.local_provider,
                slot_id,
                label,
                message: last_error,
            }),
            last_diagnostics,
        )
    }

    fn failed_item(item: FailedTranslationItem) -> TranslationItem {
        TranslationItem {
            target_lang: item.target_lang,
            text: String::new(),
            provider: item.provider_name,
            slot_id: item.slot_id,
            label: item.label,
            provider_group: Some(item.provider_group),
            experimental: item.experimental,
            local_provider: item.local_provider,
            success: false,
            error: Some(item.message),
            cached: false,
        }
    }

    fn per_attempt_timeout(
        budget_seconds: Option<f64>,
        started_at: Instant,
        max_attempts: u32,
        attempt_number: u32,
    ) -> f64 {
        let Some(budget_seconds) = budget_seconds else {
            return DEFAULT_REQUEST_TIMEOUT_SECONDS;
        };
        let elapsed = started_at.elapsed().as_secs_f64().max(0.0);
        let remaining_total = (budget_seconds - elapsed).max(0.0);
        let remaining_attempts = (max_attempts - attempt_number + 1).max(1) as f64;
        (remaining_total / remaining_attempts).max(0.25)
    }

    fn has_remaining_budget(budget_seconds: Option<f64>, started_at: Instant) -> bool {
        match budget_seconds {
            Some(budget) => (budget - started_at.elapsed().as_secs_f64()) > 0.1,
            None => true,
        }
    }

    async fn sleep_with_jitter(
        attempt_number: u32,
        budget_seconds: Option<f64>,
        started_at: Instant,
    ) {
        let base_delay = (RETRY_BACKOFF_BASE_SECONDS * 2f64.powi(attempt_number as i32 - 1))
            .min(RETRY_BACKOFF_MAX_SECONDS);
        let jitter = RETRY_BACKOFF_BASE_SECONDS * 0.5;
        let mut delay = base_delay + jitter;
        if let Some(budget) = budget_seconds {
            let remaining = budget - started_at.elapsed().as_secs_f64();
            delay = delay.min((remaining - 0.05).max(0.0));
        }
        if delay > 0.0 {
            sleep(Duration::from_secs_f64(delay)).await;
        }
    }

    fn normalized_lines(config: &Value) -> Vec<NormalizedLine> {
        let default_provider = canonical_provider_name(
            config
                .get("provider")
                .and_then(|v| v.as_str())
                .unwrap_or(DEFAULT_PROVIDER),
        );
        let mut lines = Vec::new();
        if let Some(raw_lines) = config.get("lines").and_then(|v| v.as_array()) {
            for (index, raw_line) in raw_lines.iter().enumerate() {
                let Some(obj) = raw_line.as_object() else {
                    continue;
                };
                let mut slot_id = obj
                    .get("slot_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase();
                if !CANONICAL_SLOTS.contains(&slot_id.as_str()) {
                    slot_id = CANONICAL_SLOTS
                        .get(index)
                        .unwrap_or(&"translation_1")
                        .to_string();
                }
                let target_lang = obj
                    .get("target_lang")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase();
                if slot_id.is_empty() || target_lang.is_empty() {
                    continue;
                }
                let provider = canonical_provider_name(
                    obj.get("provider")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&default_provider),
                );
                let label = obj
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                lines.push(NormalizedLine {
                    slot_id,
                    enabled: obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                    target_lang: target_lang.clone(),
                    provider,
                    label: if label.is_empty() {
                        target_lang.to_ascii_uppercase()
                    } else {
                        label
                    },
                });
            }
        }

        if lines.is_empty() {
            let legacy_targets: Vec<String> = config
                .get("target_languages")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.trim().to_ascii_lowercase())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let targets = if legacy_targets.is_empty() {
                vec!["en".into()]
            } else {
                legacy_targets
            };
            for (index, target_lang) in targets.into_iter().take(CANONICAL_SLOTS.len()).enumerate()
            {
                lines.push(NormalizedLine {
                    slot_id: CANONICAL_SLOTS[index].to_string(),
                    enabled: true,
                    target_lang: target_lang.clone(),
                    provider: default_provider.clone(),
                    label: target_lang.to_ascii_uppercase(),
                });
            }
        }

        lines.truncate(CANONICAL_SLOTS.len());
        lines
    }

    fn normalized_provider_settings(config: &Value) -> HashMap<String, HashMap<String, String>> {
        let mut out = HashMap::new();
        let Some(map) = config.get("provider_settings").and_then(|v| v.as_object()) else {
            return out;
        };
        for (provider_name, settings) in map {
            let Some(obj) = settings.as_object() else {
                continue;
            };
            let normalized = obj
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                .collect();
            out.insert(provider_name.clone(), normalized);
        }
        out
    }

    fn normalized_cache_settings(translation_config: &Value) -> CacheSettings {
        let cache = translation_config
            .get("cache")
            .and_then(|value| value.as_object());
        let enabled = cache
            .and_then(|cfg| cfg.get("enabled"))
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let persist = cache
            .and_then(|cfg| cfg.get("persist"))
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let max_entries = cache
            .and_then(|cfg| cfg.get("max_entries"))
            .and_then(|value| value.as_u64())
            .map(|value| value.clamp(0, 50_000) as usize);
        CacheSettings {
            enabled,
            persist,
            max_entries,
        }
    }

    fn build_settings_signature(translation_config: &Value) -> String {
        let mut payload = BTreeMap::new();
        payload.insert(
            "enabled".to_string(),
            translation_config
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                .to_string(),
        );
        payload.insert(
            "provider".to_string(),
            canonical_provider_name(
                translation_config
                    .get("provider")
                    .and_then(|v| v.as_str())
                    .unwrap_or(DEFAULT_PROVIDER),
            ),
        );
        for (index, line) in Self::normalized_lines(translation_config)
            .into_iter()
            .enumerate()
        {
            payload.insert(format!("line:{index}:slot_id"), line.slot_id);
            payload.insert(format!("line:{index}:enabled"), line.enabled.to_string());
            payload.insert(format!("line:{index}:target_lang"), line.target_lang);
            payload.insert(format!("line:{index}:provider"), line.provider);
            payload.insert(format!("line:{index}:label"), line.label);
        }
        let settings_map = Self::normalized_provider_settings(translation_config);
        let mut provider_names: Vec<_> = settings_map.keys().cloned().collect();
        provider_names.sort();
        for provider_name in provider_names {
            let Some(settings) = settings_map.get(&provider_name) else {
                continue;
            };
            let mut keys: Vec<_> = settings.keys().cloned().collect();
            keys.sort();
            for key in keys {
                let value = settings.get(&key).cloned().unwrap_or_default();
                payload.insert(format!("provider_setting:{provider_name}:{key}"), value);
            }
        }
        serde_json::to_string(&payload).unwrap_or_default()
    }
}
