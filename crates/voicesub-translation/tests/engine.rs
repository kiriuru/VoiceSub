use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use tempfile::TempDir;
use voicesub_translation::{
    PreparedLine, ProviderError, ProviderInfo, TranslateRequest, TranslateTargetOptions,
    TranslationBatch, TranslationEngine, TranslationProvider, DEFAULT_REQUEST_TIMEOUT_SECONDS,
};

struct FakeProvider {
    delays: HashMap<String, u64>,
    calls: Mutex<Vec<String>>,
    completions: Mutex<Vec<String>>,
    received_timeouts: Mutex<Vec<Option<f64>>>,
}

impl FakeProvider {
    fn new(delays: HashMap<String, u64>) -> Self {
        Self {
            delays,
            calls: Mutex::new(Vec::new()),
            completions: Mutex::new(Vec::new()),
            received_timeouts: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn completions(&self) -> Vec<String> {
        self.completions.lock().unwrap().clone()
    }

    fn received_timeouts(&self) -> Vec<Option<f64>> {
        self.received_timeouts.lock().unwrap().clone()
    }
}

#[async_trait]
impl TranslationProvider for FakeProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "stub",
            group: "stable",
            experimental: false,
            local_provider: false,
        }
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        self.calls
            .lock()
            .unwrap()
            .push(request.target_lang.to_string());
        self.received_timeouts
            .lock()
            .unwrap()
            .push(request.timeout_secs);
        if let Some(delay_ms) = self.delays.get(request.target_lang) {
            tokio::time::sleep(Duration::from_millis(*delay_ms)).await;
        }
        self.completions
            .lock()
            .unwrap()
            .push(request.target_lang.to_string());
        Ok(format!("{}:{}", request.text, request.target_lang))
    }

    fn diagnostics(&self, _settings: &HashMap<String, String>) -> Value {
        json!({ "provider": "stub" })
    }
}

struct FlakyProvider {
    attempts: Mutex<u32>,
}

#[async_trait]
impl TranslationProvider for FlakyProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "flaky_provider",
            group: "stable",
            experimental: false,
            local_provider: false,
        }
    }

    async fn translate(&self, _request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        let mut attempts = self.attempts.lock().unwrap();
        *attempts += 1;
        if *attempts == 1 {
            return Err(ProviderError::retryable("transient"));
        }
        Ok(format!("{}-{}", _request.text, _request.target_lang))
    }

    fn diagnostics(&self, _settings: &HashMap<String, String>) -> Value {
        json!({ "provider": "flaky_provider" })
    }
}

struct UnreliableProvider {
    attempts: Mutex<u32>,
}

#[async_trait]
impl TranslationProvider for UnreliableProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "unreliable_provider",
            group: "stable",
            experimental: false,
            local_provider: false,
        }
    }

    async fn translate(&self, _request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        *self.attempts.lock().unwrap() += 1;
        Err(ProviderError::Message("nope".into()))
    }

    fn diagnostics(&self, _settings: &HashMap<String, String>) -> Value {
        json!({ "provider": "unreliable_provider" })
    }
}

fn engine_with_fake(delays: HashMap<String, u64>, cache_dir: Option<std::path::PathBuf>) -> (TranslationEngine, Arc<FakeProvider>) {
    let fake = Arc::new(FakeProvider::new(delays));
    let mut providers = HashMap::new();
    providers.insert("stub".into(), fake.clone() as Arc<dyn TranslationProvider>);
    let engine = TranslationEngine::with_providers(providers, cache_dir);
    (engine, fake)
}

fn stub_line(target_lang: &str) -> PreparedLine {
    PreparedLine {
        slot_id: "translation_1".into(),
        target_lang: target_lang.into(),
        provider_name: "stub".into(),
        provider_settings: HashMap::new(),
        provider_group: "stable".into(),
        experimental: false,
        local_provider: false,
        label: target_lang.to_ascii_uppercase(),
    }
}

#[tokio::test]
async fn translate_targets_preserves_requested_order_with_parallel_completion() {
    let mut delays = HashMap::new();
    delays.insert("de".into(), 50);
    delays.insert("fr".into(), 10);
    delays.insert("en".into(), 30);
    let (engine, fake) = engine_with_fake(delays, None);

    let batch = engine
        .translate_targets(
            "hello",
            "ru",
            "stub",
            &HashMap::new(),
            &["de".into(), "fr".into(), "en".into()],
            0,
        )
        .await;

    assert!(matches!(batch, TranslationBatch { .. }));
    let langs: Vec<_> = batch.items.iter().map(|item| item.target_lang.as_str()).collect();
    assert_eq!(langs, vec!["de", "fr", "en"]);
    let texts: Vec<_> = batch.items.iter().map(|item| item.text.as_str()).collect();
    assert_eq!(texts, vec!["hello:de", "hello:fr", "hello:en"]);
    assert_eq!(fake.calls(), vec!["de", "fr", "en"]);
    assert_eq!(fake.completions(), vec!["fr", "en", "de"]);
}

#[tokio::test]
async fn translate_targets_uses_cache_before_calling_provider() {
    let (engine, fake) = engine_with_fake(HashMap::new(), None);
    engine.seed_translation_cache("stub", "ru", "en", "привет", "hello");

    let batch = engine
        .translate_targets(
            "привет",
            "ru",
            "stub",
            &HashMap::new(),
            &["en".into(), "fr".into()],
            0,
        )
        .await;

    assert_eq!(
        batch.items.iter().map(|i| i.target_lang.as_str()).collect::<Vec<_>>(),
        vec!["en", "fr"]
    );
    assert!(batch.items[0].cached);
    assert_eq!(batch.items[0].text, "hello");
    assert!(!batch.items[1].cached);
    assert_eq!(batch.items[1].text, "привет:fr");
    assert_eq!(fake.calls(), vec!["fr"]);
}

#[test]
fn prepare_request_uses_per_line_provider_and_duplicate_languages() {
    let engine = TranslationEngine::new(Client::new(), None);
    let prepared = engine.prepare_request(&json!({
        "enabled": true,
        "provider": "google_translate_v2",
        "target_languages": ["en"],
        "lines": [
            {
                "slot_id": "translation_1",
                "enabled": true,
                "target_lang": "en",
                "provider": "google_translate_v2",
                "label": "EN-G"
            },
            {
                "slot_id": "translation_2",
                "enabled": true,
                "target_lang": "en",
                "provider": "openai",
                "label": "EN-AI"
            },
            {
                "slot_id": "translation_3",
                "enabled": false,
                "target_lang": "ja",
                "provider": "deepl",
                "label": "JA"
            }
        ],
        "provider_settings": {
            "google_translate_v2": { "api_key": "AIza-demo" },
            "openai": {
                "api_key": "sk-demo",
                "model": "gpt-4o-mini",
                "base_url": "https://api.openai.com/v1"
            }
        }
    }));

    assert_eq!(prepared.provider_name, "mixed");
    assert_eq!(
        prepared.lines.iter().map(|l| l.slot_id.as_str()).collect::<Vec<_>>(),
        vec!["translation_1", "translation_2"]
    );
    assert_eq!(
        prepared.lines.iter().map(|l| l.target_lang.as_str()).collect::<Vec<_>>(),
        vec!["en", "en"]
    );
    assert_eq!(
        prepared.lines.iter().map(|l| l.provider_name.as_str()).collect::<Vec<_>>(),
        vec!["google_translate_v2", "openai"]
    );
    assert_eq!(prepared.lines[0].provider_settings["api_key"], "AIza-demo");
    assert_eq!(prepared.lines[1].provider_settings["model"], "gpt-4o-mini");
    assert_eq!(prepared.lines[1].label, "EN-AI");
}

#[tokio::test]
async fn translate_target_cache_key_includes_provider_name() {
    let fake = Arc::new(FakeProvider::new(HashMap::new()));
    let mut providers = HashMap::new();
    providers.insert(
        "google_translate_v2".into(),
        fake.clone() as Arc<dyn TranslationProvider>,
    );
    providers.insert("openai".into(), fake.clone() as Arc<dyn TranslationProvider>);
    let mut engine = TranslationEngine::with_providers(providers, None);
    engine.seed_translation_cache("google_translate_v2", "en", "fr", "hello", "cached-google");
    engine.seed_translation_cache("openai", "en", "fr", "hello", "cached-openai");

    let google_line = PreparedLine {
        slot_id: "translation_1".into(),
        target_lang: "fr".into(),
        provider_name: "google_translate_v2".into(),
        provider_settings: HashMap::new(),
        provider_group: "stable".into(),
        experimental: false,
        local_provider: false,
        label: "FR".into(),
    };
    let openai_line = PreparedLine {
        provider_name: "openai".into(),
        ..google_line.clone()
    };

    let (google_item, _) = engine
        .translate_target("hello", "en", &google_line, TranslateTargetOptions::default())
        .await;
    let (openai_item, _) = engine
        .translate_target("hello", "en", &openai_line, TranslateTargetOptions::default())
        .await;

    assert_eq!(google_item.text, "cached-google");
    assert_eq!(openai_item.text, "cached-openai");
    assert!(google_item.cached);
    assert!(openai_item.cached);
}

#[tokio::test]
async fn translate_target_short_circuits_for_empty_source_text() {
    let (mut engine, fake) = engine_with_fake(HashMap::new(), None);
    let line = stub_line("fr");

    let (item, diagnostics) = engine
        .translate_target("   ", "en", &line, TranslateTargetOptions::default())
        .await;

    assert!(item.success);
    assert!(item.cached);
    assert_eq!(item.text, "   ");
    assert!(fake.calls().is_empty());
    let status = diagnostics
        .get("status_message")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(status.to_ascii_lowercase().contains("short-circuited"));
}

#[tokio::test]
async fn translate_target_short_circuits_when_source_and_target_match() {
    let (mut engine, fake) = engine_with_fake(HashMap::new(), None);
    let line = stub_line("en");

    let (item, _) = engine
        .translate_target("hello world", "EN", &line, TranslateTargetOptions::default())
        .await;

    assert!(item.success);
    assert!(item.cached);
    assert_eq!(item.text, "hello world");
    assert!(fake.calls().is_empty());
}

#[tokio::test]
async fn translate_target_does_not_short_circuit_when_source_is_auto() {
    let (mut engine, fake) = engine_with_fake(HashMap::new(), None);
    let line = stub_line("en");

    let (item, _) = engine
        .translate_target("hello", "auto", &line, TranslateTargetOptions::default())
        .await;

    assert_eq!(fake.calls(), vec!["en"]);
    assert_eq!(item.text, "hello:en");
}

#[tokio::test]
async fn translate_target_propagates_budget_to_provider() {
    let (mut engine, fake) = engine_with_fake(HashMap::new(), None);
    let line = stub_line("fr");

    engine
        .translate_target(
            "hello",
            "en",
            &line,
            TranslateTargetOptions {
                budget_seconds: Some(4.0),
                retries: 1,
                ..Default::default()
            },
        )
        .await;

    let timeouts = fake.received_timeouts();
    assert_eq!(timeouts.len(), 1);
    let timeout = timeouts[0].unwrap();
    assert!(timeout > 0.25);
    assert!(timeout <= 4.0);
}

#[tokio::test]
async fn translate_target_uses_default_timeout_when_no_budget_given() {
    let (mut engine, fake) = engine_with_fake(HashMap::new(), None);
    let line = stub_line("fr");

    engine
        .translate_target("hello", "en", &line, TranslateTargetOptions::default())
        .await;

    assert_eq!(fake.received_timeouts(), vec![Some(DEFAULT_REQUEST_TIMEOUT_SECONDS)]);
}

#[tokio::test]
async fn apply_live_settings_propagates_cache_toggle() {
    let temp = TempDir::new().unwrap();
    let fake = Arc::new(FakeProvider::new(HashMap::new()));
    let mut providers = HashMap::new();
    providers.insert(
        "google_translate_v2".into(),
        fake as Arc<dyn TranslationProvider>,
    );
    let mut engine = TranslationEngine::with_providers(providers, Some(temp.path().to_path_buf()));
    engine.seed_translation_cache("google_translate_v2", "en", "fr", "hello", "bonjour");

    engine.apply_live_settings(&json!({
        "enabled": true,
        "provider": "google_translate_v2",
        "lines": [{
            "slot_id": "translation_1",
            "enabled": true,
            "target_lang": "fr",
            "provider": "google_translate_v2"
        }],
        "cache": { "enabled": false, "persist": false }
    }));

    let line = PreparedLine {
        slot_id: "translation_1".into(),
        target_lang: "fr".into(),
        provider_name: "google_translate_v2".into(),
        provider_settings: HashMap::new(),
        provider_group: "stable".into(),
        experimental: false,
        local_provider: false,
        label: "FR".into(),
    };
    let (item, _) = engine
        .translate_target("hello", "en", &line, TranslateTargetOptions::default())
        .await;

    assert!(!item.cached);
    assert_eq!(item.text, "hello:fr");
}

#[tokio::test]
async fn translate_with_retry_retries_on_retryable_failure_and_then_succeeds() {
    let flaky = Arc::new(FlakyProvider {
        attempts: Mutex::new(0),
    });
    let mut providers = HashMap::new();
    providers.insert("stub".into(), flaky.clone() as Arc<dyn TranslationProvider>);
    let mut engine = TranslationEngine::with_providers(providers, None);
    let line = PreparedLine {
        slot_id: "translation_1".into(),
        target_lang: "fr".into(),
        provider_name: "stub".into(),
        provider_settings: HashMap::new(),
        provider_group: "stable".into(),
        experimental: false,
        local_provider: false,
        label: "FR".into(),
    };

    let (item, _) = engine
        .translate_target(
            "hi",
            "en",
            &line,
            TranslateTargetOptions {
                budget_seconds: Some(5.0),
                retries: 2,
                ..Default::default()
            },
        )
        .await;

    assert!(item.success);
    assert_eq!(item.text, "hi-fr");
    assert_eq!(*flaky.attempts.lock().unwrap(), 2);
}

#[tokio::test]
async fn translate_with_retry_does_not_retry_non_retryable_failure() {
    let unreliable = Arc::new(UnreliableProvider {
        attempts: Mutex::new(0),
    });
    let mut providers = HashMap::new();
    providers.insert(
        "stub".into(),
        unreliable.clone() as Arc<dyn TranslationProvider>,
    );
    let mut engine = TranslationEngine::with_providers(providers, None);
    let line = PreparedLine {
        slot_id: "translation_1".into(),
        target_lang: "fr".into(),
        provider_name: "stub".into(),
        provider_settings: HashMap::new(),
        provider_group: "stable".into(),
        experimental: false,
        local_provider: false,
        label: "FR".into(),
    };

    let (item, _) = engine
        .translate_target(
            "hi",
            "en",
            &line,
            TranslateTargetOptions {
                budget_seconds: Some(5.0),
                retries: 3,
                ..Default::default()
            },
        )
        .await;

    assert!(!item.success);
    assert_eq!(*unreliable.attempts.lock().unwrap(), 1);
}

#[tokio::test]
async fn engine_binds_shared_http_client_provider_to_registered_providers() {
    let client = reqwest::Client::new();
    let engine = TranslationEngine::new(client, None);
    assert!(engine.http_transport().is_bound());
    assert!(engine.provider("google_translate_v2").is_some());
}

#[tokio::test]
async fn google_cloud_translation_v3_uses_advanced_endpoint_and_bearer_token() {
    use std::collections::HashMap;

    use voicesub_translation::{
        GoogleCloudTranslationV3Provider, SharedHttpClient, TranslateRequest, TranslationProvider,
    };
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(
            "/v3/projects/demo-project/locations/global:translateText",
        ))
        .and(header("Authorization", "Bearer ya29.token-value"))
        .and(header("x-goog-user-project", "demo-project"))
        .and(body_json(serde_json::json!({
            "contents": ["привет"],
            "targetLanguageCode": "en",
            "sourceLanguageCode": "ru",
            "mimeType": "text/plain",
            "model": "general/nmt"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "translations": [{ "translatedText": "hello" }]
        })))
        .mount(&server)
        .await;

    let transport = SharedHttpClient::new(reqwest::Client::new());
    let provider = GoogleCloudTranslationV3Provider::new(transport)
        .with_api_root_for_test(server.uri());

    let settings = HashMap::from([
        ("project_id".into(), "demo-project".into()),
        ("access_token".into(), "ya29.token-value".into()),
        ("location".into(), "global".into()),
        ("model".into(), "general/nmt".into()),
    ]);
    let request = TranslateRequest {
        text: "привет",
        source_lang: "ru",
        target_lang: "en",
        settings: &settings,
        timeout_secs: None,
    };

    let translated = provider.translate(request).await.expect("translate");
    assert_eq!(translated, "hello");

    let diagnostics = provider.diagnostics(&settings);
    assert_eq!(diagnostics["provider"], "google_cloud_translation_v3");
    assert_eq!(diagnostics["location"], "global");
}
