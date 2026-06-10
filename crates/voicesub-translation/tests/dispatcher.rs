use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use voicesub_subtitle::TranslationEvent;
use voicesub_translation::{
    arc_publish, arc_relevance, build_default_registry, ConfigGetter, DispatcherCallbacks,
    MetricsCallback, SharedHttpClient, StructuredLogFn, StubTranslationProvider,
    TranslationDispatcher, TranslationEngine, TranslationProvider,
};

fn stub_config(extra: Value) -> Value {
    json!({
        "translation": {
            "enabled": true,
            "provider": "stub",
            "target_languages": ["en"],
            "lines": [{
                "slot_id": "translation_1",
                "enabled": true,
                "target_lang": "en",
                "provider": "stub",
                "label": "EN"
            }],
            "timeout_ms": 500,
            "max_concurrent_jobs": 2
        }
    })
    .merge(extra)
}

trait MergeJson {
    fn merge(self, other: Value) -> Value;
}

impl MergeJson for Value {
    fn merge(mut self, other: Value) -> Value {
        merge_values(&mut self, other);
        self
    }
}

fn merge_values(base: &mut Value, patch: Value) {
    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            for (key, value) in patch_map {
                match base_map.get_mut(&key) {
                    Some(existing) => merge_values(existing, value),
                    None => {
                        base_map.insert(key, value);
                    }
                }
            }
        }
        (base_slot, patch_val) => *base_slot = patch_val,
    }
}

struct RecordingPublisher {
    events: Arc<Mutex<Vec<TranslationEvent>>>,
}

impl RecordingPublisher {
    fn new() -> (Self, voicesub_translation::PublishFn) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_cb = events.clone();
        let publish = arc_publish(move |event| {
            let events = events_cb.clone();
            async move {
                events.lock().unwrap().push(event);
            }
        });
        (Self { events }, publish)
    }
}

struct RelevanceSet {
    sequences: Arc<Mutex<HashSet<u64>>>,
}

impl RelevanceSet {
    fn new(initial: &[u64]) -> (Self, voicesub_translation::RelevanceFn) {
        let sequences = Arc::new(Mutex::new(
            initial.iter().copied().collect::<HashSet<u64>>(),
        ));
        let sequences_cb = sequences.clone();
        let relevance = arc_relevance(move |sequence| {
            let sequences = sequences_cb.clone();
            async move { sequences.lock().unwrap().contains(&sequence) }
        });
        (Self { sequences }, relevance)
    }

    fn clear(&self) {
        self.sequences.lock().unwrap().clear();
    }

    fn insert(&self, sequence: u64) {
        self.sequences.lock().unwrap().insert(sequence);
    }
}

struct StructuredLogger {
    records: Arc<Mutex<Vec<Value>>>,
}

impl StructuredLogger {
    fn new() -> (Self, StructuredLogFn) {
        let records = Arc::new(Mutex::new(Vec::new()));
        let records_cb = records.clone();
        let logger: StructuredLogFn = Arc::new(move |channel, event, payload| {
            records_cb.lock().unwrap().push(json!({
                "channel": channel,
                "event": event,
                "payload": payload,
            }));
        });
        (Self { records }, logger)
    }
}

struct MetricsRecorder {
    snapshots: Arc<Mutex<Vec<Value>>>,
}

impl MetricsRecorder {
    fn new() -> (Self, MetricsCallback) {
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        let snapshots_cb = snapshots.clone();
        let callback: MetricsCallback = Arc::new(move |snapshot| {
            snapshots_cb.lock().unwrap().push(snapshot);
        });
        (Self { snapshots }, callback)
    }
}

fn test_engine(client: reqwest::Client) -> TranslationEngine {
    let transport = SharedHttpClient::new(client);
    let stub = Arc::new(StubTranslationProvider::new(transport.clone()));
    let mut providers = build_default_registry(transport);
    for name in [
        "stub",
        "google_translate_v2",
        "openai",
        "deepl",
        "libretranslate",
    ] {
        providers.insert(name.into(), stub.clone() as Arc<dyn TranslationProvider>);
    }
    TranslationEngine::with_providers(providers, None)
}

#[test]
fn stub_config_deep_merges_translation_sections() {
    let config = stub_config(json!({
        "translation": {
            "provider_limits": { "stub": { "max_concurrent_targets": 1 } },
            "provider_settings": { "stub": { "delay_ms_translation_1": "1500" } },
            "timeout_ms": 1000
        }
    }));
    assert_eq!(
        config["translation"]["provider_limits"]["stub"]["max_concurrent_targets"].as_u64(),
        Some(1)
    );
    assert_eq!(
        config["translation"]["provider_settings"]["stub"]["delay_ms_translation_1"].as_str(),
        Some("1500")
    );
    assert_eq!(config["translation"]["timeout_ms"].as_u64(), Some(1000));
    assert!(config["translation"]["enabled"].as_bool().unwrap_or(false));
}

#[tokio::test]
async fn stub_provider_honors_delay_settings() {
    let config = stub_config(json!({
        "translation": {
            "provider_settings": { "stub": { "delay_ms_translation_1": "250" } }
        }
    }));
    let mut engine = test_engine(reqwest::Client::new());
    let translation = config["translation"].clone();
    engine.apply_live_settings(&translation);
    let prepared = engine.prepare_request(&translation);
    let line = &prepared.lines[0];
    let started = Instant::now();
    engine
        .translate_target(
            "hello",
            "ru",
            line,
            voicesub_translation::TranslateTargetOptions {
                slot_id: Some("translation_1".into()),
                label: Some("EN".into()),
                ..Default::default()
            },
        )
        .await;
    assert!(
        started.elapsed() >= Duration::from_millis(200),
        "elapsed={:?}",
        started.elapsed()
    );
}

fn make_dispatcher(
    config: Value,
    publish: voicesub_translation::PublishFn,
    relevance: voicesub_translation::RelevanceFn,
) -> Arc<TranslationDispatcher> {
    make_dispatcher_with_callbacks(config, publish, relevance, DispatcherCallbacks::default())
}

fn make_dispatcher_with_callbacks(
    config: Value,
    publish: voicesub_translation::PublishFn,
    relevance: voicesub_translation::RelevanceFn,
    callbacks: DispatcherCallbacks,
) -> Arc<TranslationDispatcher> {
    let engine = test_engine(reqwest::Client::new());
    TranslationDispatcher::with_callbacks(
        engine,
        Arc::new(move || config.clone()),
        publish,
        relevance,
        callbacks,
    )
}

#[tokio::test]
async fn dispatcher_publishes_incremental_and_completion_events() {
    let config = stub_config(json!({}));
    let config_getter: ConfigGetter = Arc::new(move || config.clone());
    let (recorder, publish) = RecordingPublisher::new();
    let (_, relevance) = RelevanceSet::new(&[1]);
    let dispatcher = TranslationDispatcher::new(
        TranslationEngine::new_stub(reqwest::Client::new()),
        config_getter,
        publish,
        relevance,
    );
    dispatcher.start().await;
    dispatcher.submit_final(1, "hello", "en", None).await;
    tokio::time::sleep(Duration::from_millis(120)).await;
    dispatcher.stop().await;

    let events = recorder.events.lock().unwrap().clone();
    assert!(!events.is_empty());
    assert!(events.iter().any(|event| !event.is_complete));
    assert!(events
        .last()
        .map(|event| event.is_complete)
        .unwrap_or(false));
    assert_eq!(events.last().unwrap().sequence, 1);
}

#[tokio::test]
async fn dispatcher_publishes_fresh_translation_with_stub() {
    let config = stub_config(json!({}));
    let (recorder, publish) = RecordingPublisher::new();
    let (_, relevance) = RelevanceSet::new(&[1]);
    let dispatcher = make_dispatcher(config, publish, relevance);
    dispatcher.start().await;
    dispatcher.submit_final(1, "hello", "ru", None).await;
    tokio::time::sleep(Duration::from_millis(120)).await;
    dispatcher.stop().await;

    let events = recorder.events.lock().unwrap().clone();
    let partial = events
        .iter()
        .find(|event| !event.is_complete)
        .expect("incremental");
    assert_eq!(partial.translations[0].text, "hello-en");
    assert_eq!(
        partial.translations[0].slot_id.as_deref(),
        Some("translation_1")
    );
}

#[tokio::test]
async fn dispatcher_drops_stale_translation_result() {
    let config = stub_config(json!({
        "translation": {
            "enabled": true,
            "provider": "stub",
            "timeout_ms": 500,
            "max_concurrent_jobs": 2,
            "provider_settings": {
                "stub": { "delay_ms_translation_1": "200" }
            }
        }
    }));
    let (recorder, publish) = RecordingPublisher::new();
    let (relevance, relevance_fn) = RelevanceSet::new(&[1]);
    let dispatcher = make_dispatcher(config, publish, relevance_fn);
    dispatcher.start().await;
    dispatcher.submit_final(1, "hello", "ru", None).await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    relevance.clear();
    tokio::time::sleep(Duration::from_millis(250)).await;
    dispatcher.stop().await;

    assert!(recorder.events.lock().unwrap().is_empty());
    assert!(
        dispatcher
            .metrics_snapshot()
            .get("translation_stale_results_dropped")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
            >= 1
    );
}

#[tokio::test]
async fn dispatcher_slow_target_does_not_block_fast_target() {
    let config = stub_config(json!({
        "translation": {
            "enabled": true,
            "provider": "stub",
            "timeout_ms": 500,
            "max_concurrent_jobs": 2,
            "target_languages": ["en", "de"],
            "lines": [
                {
                    "slot_id": "translation_1",
                    "enabled": true,
                    "target_lang": "en",
                    "provider": "stub",
                    "label": "EN"
                },
                {
                    "slot_id": "translation_2",
                    "enabled": true,
                    "target_lang": "de",
                    "provider": "stub",
                    "label": "DE"
                }
            ],
            "provider_settings": {
                "stub": {
                    "delay_ms_translation_1": "10",
                    "delay_ms_translation_2": "200"
                }
            }
        }
    }));
    let (recorder, publish) = RecordingPublisher::new();
    let (_, relevance) = RelevanceSet::new(&[1]);
    let dispatcher = make_dispatcher(config, publish, relevance);
    dispatcher.start().await;
    dispatcher.submit_final(1, "hello", "ru", None).await;

    let deadline = tokio::time::Instant::now() + Duration::from_millis(400);
    loop {
        let has_partial = recorder
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|event| !event.translations.is_empty());
        if has_partial || tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    dispatcher.stop().await;

    let events = recorder.events.lock().unwrap().clone();
    let first_slots = events
        .iter()
        .filter(|event| !event.translations.is_empty())
        .map(|event| event.translations[0].slot_id.as_deref().unwrap_or(""))
        .collect::<Vec<_>>();
    assert_eq!(first_slots, vec!["translation_1"]);
}

#[tokio::test]
async fn dispatcher_can_restart_after_stop() {
    let config = stub_config(json!({}));
    let (recorder, publish) = RecordingPublisher::new();
    let (relevance, relevance_fn) = RelevanceSet::new(&[1]);
    let dispatcher = make_dispatcher(config, publish, relevance_fn);
    dispatcher.start().await;
    dispatcher.submit_final(1, "hello", "en", None).await;
    tokio::time::sleep(Duration::from_millis(120)).await;
    dispatcher.stop().await;

    recorder.events.lock().unwrap().clear();
    relevance.clear();
    relevance.insert(2);
    dispatcher.start().await;
    dispatcher.submit_final(2, "again", "en", None).await;
    tokio::time::sleep(Duration::from_millis(120)).await;
    dispatcher.stop().await;

    assert!(recorder
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|event| event.sequence == 2 && !event.translations.is_empty()));
}

#[tokio::test]
async fn dispatcher_completion_keeps_all_published_translations() {
    let config = stub_config(json!({
        "translation": {
            "lines": [
                {
                    "slot_id": "translation_1",
                    "enabled": true,
                    "target_lang": "en",
                    "provider": "stub",
                    "label": "EN"
                },
                {
                    "slot_id": "translation_2",
                    "enabled": true,
                    "target_lang": "de",
                    "provider": "stub",
                    "label": "DE"
                }
            ],
            "provider_settings": {
                "stub": {
                    "delay_ms_translation_1": "10",
                    "delay_ms_translation_2": "20"
                }
            }
        }
    }));
    let (recorder, publish) = RecordingPublisher::new();
    let (_, relevance) = RelevanceSet::new(&[12]);
    let dispatcher = make_dispatcher(config, publish, relevance);
    dispatcher.start().await;
    dispatcher.submit_final(12, "hello", "en", None).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    dispatcher.stop().await;

    let events = recorder.events.lock().unwrap().clone();
    let complete = events
        .iter()
        .find(|event| event.sequence == 12 && event.is_complete)
        .expect("completion");
    assert_eq!(
        complete
            .translations
            .iter()
            .map(|item| item.target_lang.as_str())
            .collect::<Vec<_>>(),
        vec!["en", "de"]
    );
    assert_eq!(
        complete
            .translations
            .iter()
            .map(|item| item.slot_id.as_deref().unwrap_or(""))
            .collect::<Vec<_>>(),
        vec!["translation_1", "translation_2"]
    );
}

#[tokio::test]
async fn dispatcher_skips_superseded_preview_with_concurrent_jobs() {
    let config = stub_config(json!({
        "translation": {
            "max_concurrent_jobs": 2,
            "provider_settings": {
                "stub": { "delay_ms_translation_1": "20000" }
            }
        }
    }));
    let (recorder, publish) = RecordingPublisher::new();
    let (_, relevance) = RelevanceSet::new(&[1, 2]);
    let dispatcher = make_dispatcher(config, publish, relevance);
    dispatcher.start().await;
    let key = "seg:rev:1";
    tokio::join!(
        dispatcher.submit_final(1, "older", "en", Some(key)),
        dispatcher.submit_final(2, "newer", "en", Some(key)),
    );
    tokio::time::sleep(Duration::from_millis(350)).await;
    dispatcher.stop().await;

    let events = recorder.events.lock().unwrap().clone();
    assert!(events.iter().all(|event| event.sequence == 2));
    assert!(!events.is_empty());
}

#[tokio::test]
async fn dispatcher_cancel_older_than_cancels_irrelevant_jobs() {
    let config = stub_config(json!({
        "translation": {
            "timeout_ms": 2000,
            "provider_settings": {
                "stub": { "delay_ms_translation_1": "500" }
            }
        }
    }));
    let (recorder, publish) = RecordingPublisher::new();
    let (relevance, relevance_fn) = RelevanceSet::new(&[1]);
    let (structured, structured_log) = StructuredLogger::new();
    let dispatcher = make_dispatcher_with_callbacks(
        config,
        publish,
        relevance_fn,
        DispatcherCallbacks {
            structured_log: Some(structured_log),
            metrics_callback: None,
        },
    );
    dispatcher.start().await;
    dispatcher.submit_final(1, "hello", "ru", None).await;
    tokio::time::sleep(Duration::from_millis(80)).await;
    relevance.clear();
    relevance.insert(2);
    dispatcher.cancel_older_than(2).await;
    dispatcher.submit_final(2, "new", "ru", None).await;

    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        let has_seq2 = recorder
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event.sequence == 2);
        if has_seq2 || Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    dispatcher.stop().await;

    let events = recorder.events.lock().unwrap().clone();
    assert!(events.iter().any(|event| event.sequence == 2));
    assert!(!events.iter().any(|event| event.sequence == 1));

    let records = structured.records.lock().unwrap().clone();
    let cancel_events: Vec<_> = records
        .iter()
        .filter(|record| {
            record.get("event").and_then(|v| v.as_str()) == Some("translation_job_cancelled")
        })
        .collect();
    assert!(
        !cancel_events.is_empty(),
        "expected translation_job_cancelled structured log"
    );
    assert!(cancel_events.iter().any(|record| {
        record
            .get("payload")
            .and_then(|payload| payload.get("reason"))
            .and_then(|v| v.as_str())
            == Some("active_job_replaced")
    }));
}

#[tokio::test]
async fn dispatcher_frees_concurrency_when_long_running_job_cancelled() {
    let config = stub_config(json!({
        "translation": {
            "max_concurrent_jobs": 1,
            "timeout_ms": 20000,
            "provider_settings": {
                "stub": { "delay_ms_translation_1": "20000" }
            }
        }
    }));
    let (_recorder, publish) = RecordingPublisher::new();
    let (relevance, relevance_fn) = RelevanceSet::new(&[1]);
    let (structured, structured_log) = StructuredLogger::new();
    let dispatcher = make_dispatcher_with_callbacks(
        config,
        publish,
        relevance_fn,
        DispatcherCallbacks {
            structured_log: Some(structured_log),
            metrics_callback: None,
        },
    );
    dispatcher.start().await;
    dispatcher.submit_final(1, "stuck", "ru", None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    relevance.clear();
    relevance.insert(2);
    dispatcher.cancel_older_than(2).await;
    dispatcher.submit_final(2, "fresh", "ru", None).await;

    let deadline = Instant::now() + Duration::from_millis(800);
    loop {
        let started = structured
            .records
            .lock()
            .unwrap()
            .iter()
            .any(|record| {
                record.get("event").and_then(|v| v.as_str()) == Some("translation_job_started")
                    && record
                        .get("payload")
                        .and_then(|payload| payload.get("sequence"))
                        .and_then(|v| v.as_u64())
                        == Some(2)
            });
        if started || Instant::now() >= deadline {
            assert!(
                started,
                "sequence 2 job should start after cancelling stuck job 1"
            );
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    dispatcher.stop().await;
}

#[tokio::test]
async fn dispatcher_emits_structured_events_without_secrets() {
    let config = stub_config(json!({
        "translation": { "api_key": "top-secret-key" }
    }));
    let (_recorder, publish) = RecordingPublisher::new();
    let (_, relevance) = RelevanceSet::new(&[7]);
    let (structured, structured_log) = StructuredLogger::new();
    let (metrics, metrics_callback) = MetricsRecorder::new();
    let dispatcher = make_dispatcher_with_callbacks(
        config,
        publish,
        relevance,
        DispatcherCallbacks {
            structured_log: Some(structured_log),
            metrics_callback: Some(metrics_callback),
        },
    );
    dispatcher.start().await;
    dispatcher
        .submit_final(7, "do not leak this sentence", "en", None)
        .await;
    tokio::time::sleep(Duration::from_millis(120)).await;
    dispatcher.stop().await;

    let records = structured.records.lock().unwrap().clone();
    let events: Vec<_> = records
        .iter()
        .filter_map(|record| record.get("event").and_then(|v| v.as_str()))
        .collect();
    assert!(events.contains(&"translation_job_started"));
    assert!(events.contains(&"translation_line_started"));
    assert!(events.contains(&"translation_line_done"));
    assert!(events.contains(&"translation_publish_accepted"));
    let serialized = serde_json::to_string(&records).unwrap();
    assert!(!serialized.contains("do not leak this sentence"));
    assert!(!serialized.contains("top-secret-key"));
    assert!(serialized.contains("source_text_len"));
    assert!(!metrics.snapshots.lock().unwrap().is_empty());
}

#[tokio::test]
async fn dispatcher_timeout_emits_structured_event_and_still_completes() {
    let config = stub_config(json!({
        "translation": {
            "timeout_ms": 1000,
            "provider_settings": {
                "stub": { "delay_ms_translation_1": "1500" }
            }
        }
    }));
    let (recorder, publish) = RecordingPublisher::new();
    let (_, relevance) = RelevanceSet::new(&[8]);
    let (structured, structured_log) = StructuredLogger::new();
    let dispatcher = make_dispatcher_with_callbacks(
        config,
        publish,
        relevance,
        DispatcherCallbacks {
            structured_log: Some(structured_log),
            metrics_callback: None,
        },
    );
    dispatcher.start().await;
    dispatcher.submit_final(8, "slow target", "ru", None).await;
    tokio::time::sleep(Duration::from_millis(1350)).await;
    dispatcher.stop().await;

    let events = recorder.events.lock().unwrap().clone();
    let partials: Vec<_> = events
        .iter()
        .filter(|event| event.sequence == 8 && !event.is_complete)
        .collect();
    assert_eq!(partials.len(), 1);
    assert!(!partials[0].translations[0].success);
    let complete = events
        .iter()
        .find(|event| event.sequence == 8 && event.is_complete)
        .expect("completion");
    assert_eq!(complete.translations.len(), 1);
    assert!(!complete.translations[0].success);

    let records = structured.records.lock().unwrap().clone();
    assert!(records
        .iter()
        .any(|record| record.get("event").and_then(|v| v.as_str())
            == Some("translation_line_timeout")));
}

#[tokio::test]
async fn dispatcher_mixed_provider_completion_is_mixed() {
    let config = stub_config(json!({
        "translation": {
            "provider": "google_translate_v2",
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
                }
            ],
            "provider_settings": {
                "stub": {
                    "delay_ms_translation_1": "10",
                    "delay_ms_translation_2": "20"
                },
                "google_translate_v2": {
                    "delay_ms_translation_1": "10"
                },
                "openai": {
                    "delay_ms_translation_2": "20"
                }
            }
        }
    }));
    let (recorder, publish) = RecordingPublisher::new();
    let (_, relevance) = RelevanceSet::new(&[21]);
    let (metrics, metrics_callback) = MetricsRecorder::new();
    let dispatcher = make_dispatcher_with_callbacks(
        config,
        publish,
        relevance,
        DispatcherCallbacks {
            structured_log: None,
            metrics_callback: Some(metrics_callback),
        },
    );
    dispatcher.start().await;
    dispatcher.submit_final(21, "hello", "en", None).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    dispatcher.stop().await;

    let events = recorder.events.lock().unwrap().clone();
    let partial_slots: Vec<_> = events
        .iter()
        .filter(|event| event.sequence == 21 && !event.is_complete)
        .map(|event| event.translations[0].slot_id.as_deref().unwrap_or(""))
        .collect();
    assert_eq!(partial_slots, vec!["translation_1", "translation_2"]);
    let partial_providers: Vec<_> = events
        .iter()
        .filter(|event| event.sequence == 21 && !event.is_complete)
        .map(|event| event.translations[0].provider.as_str())
        .collect();
    assert_eq!(partial_providers, vec!["google_translate_v2", "openai"]);
    let complete = events
        .iter()
        .find(|event| event.sequence == 21 && event.is_complete)
        .expect("completion");
    assert_eq!(complete.provider, "mixed");
    assert_eq!(
        metrics
            .snapshots
            .lock()
            .unwrap()
            .last()
            .and_then(|snapshot| snapshot.get("translation_last_slot_id"))
            .and_then(|value| value.as_str()),
        Some("translation_2")
    );
}

#[tokio::test]
async fn dispatcher_one_line_failure_does_not_block_other_line() {
    let config = stub_config(json!({
        "translation": {
            "lines": [
                {
                    "slot_id": "translation_1",
                    "enabled": true,
                    "target_lang": "en",
                    "provider": "stub",
                    "label": "EN"
                },
                {
                    "slot_id": "translation_2",
                    "enabled": true,
                    "target_lang": "de",
                    "provider": "stub",
                    "label": "DE"
                }
            ],
            "provider_settings": {
                "stub": { "fail_slot": "translation_2" }
            }
        }
    }));
    let (recorder, publish) = RecordingPublisher::new();
    let (_, relevance) = RelevanceSet::new(&[22]);
    let dispatcher = make_dispatcher(config, publish, relevance);
    dispatcher.start().await;
    dispatcher.submit_final(22, "hello", "en", None).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    dispatcher.stop().await;

    let events = recorder.events.lock().unwrap().clone();
    let partials: Vec<_> = events
        .iter()
        .filter(|event| event.sequence == 22 && !event.is_complete)
        .collect();
    assert_eq!(partials.len(), 2);
    let by_slot: HashMap<_, _> = partials
        .iter()
        .map(|event| {
            (
                event.translations[0].slot_id.as_deref().unwrap_or(""),
                &event.translations[0],
            )
        })
        .collect();
    assert!(by_slot["translation_1"].success);
    assert!(!by_slot["translation_2"].success);
    assert_eq!(
        by_slot["translation_2"].error.as_deref(),
        Some("translation_2 exploded")
    );
    let complete = events
        .iter()
        .find(|event| event.sequence == 22 && event.is_complete)
        .expect("completion");
    let slots: HashSet<_> = complete
        .translations
        .iter()
        .filter_map(|item| item.slot_id.as_deref())
        .collect();
    assert_eq!(slots, HashSet::from(["translation_1", "translation_2"]));
}

#[tokio::test]
async fn dispatcher_prepare_failure_emits_structured_job_error() {
    let config = stub_config(json!({
        "translation": {
            "__test_fail_prepare": true,
            "__test_fail_prepare_message": "prepare_request exploded"
        }
    }));
    let (recorder, publish) = RecordingPublisher::new();
    let (_, relevance) = RelevanceSet::new(&[11]);
    let (structured, structured_log) = StructuredLogger::new();
    let (metrics, metrics_callback) = MetricsRecorder::new();
    let dispatcher = make_dispatcher_with_callbacks(
        config,
        publish,
        relevance,
        DispatcherCallbacks {
            structured_log: Some(structured_log),
            metrics_callback: Some(metrics_callback),
        },
    );
    dispatcher.start().await;
    dispatcher.submit_final(11, "boom", "en", None).await;
    tokio::time::sleep(Duration::from_millis(80)).await;
    dispatcher.stop().await;

    assert!(recorder.events.lock().unwrap().is_empty());
    let records = structured.records.lock().unwrap().clone();
    let job_errors: Vec<_> = records
        .iter()
        .filter(|record| {
            record.get("event").and_then(|v| v.as_str()) == Some("translation_job_error")
        })
        .collect();
    assert_eq!(job_errors.len(), 1);
    assert_eq!(
        job_errors[0]
            .get("payload")
            .and_then(|payload| payload.get("sequence"))
            .and_then(|value| value.as_u64()),
        Some(11)
    );
    assert_eq!(
        metrics
            .snapshots
            .lock()
            .unwrap()
            .last()
            .and_then(|snapshot| snapshot.get("translation_last_runtime_reason"))
            .and_then(|value| value.as_str()),
        Some("job_error:prepare_request exploded")
    );
}

#[tokio::test]
async fn dispatcher_provider_concurrency_limit_serializes_targets() {
    let config = stub_config(json!({
        "translation": {
            "provider_limits": { "stub": { "max_concurrent_targets": 1 } },
            "lines": [
                {
                    "slot_id": "translation_1",
                    "enabled": true,
                    "target_lang": "en",
                    "provider": "stub",
                    "label": "EN"
                },
                {
                    "slot_id": "translation_2",
                    "enabled": true,
                    "target_lang": "de",
                    "provider": "stub",
                    "label": "DE"
                }
            ],
            "provider_settings": {
                "stub": {
                    "delay_ms_translation_1": "120",
                    "delay_ms_translation_2": "120"
                }
            }
        }
    }));
    let (recorder, publish) = RecordingPublisher::new();
    let (_, relevance) = RelevanceSet::new(&[30]);
    let dispatcher = make_dispatcher(config, publish, relevance);
    dispatcher.start().await;
    let started = Instant::now();
    dispatcher.submit_final(30, "hello", "ru", None).await;
    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        let partial_count = recorder
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| !event.is_complete)
            .count();
        if partial_count >= 2 || Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    dispatcher.stop().await;
    assert!(
        started.elapsed() >= Duration::from_millis(200),
        "expected serialized provider calls, elapsed={:?}",
        started.elapsed()
    );
}
