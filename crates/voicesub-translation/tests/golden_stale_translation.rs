use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::Value;
use voicesub_translation::{
    arc_publish, arc_relevance, ConfigGetter, TranslationDispatcher, TranslationEngine,
};

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("tests/golden/translation/drops_stale_translation.json")
}

#[derive(Debug, Deserialize)]
struct GoldenFixture {
    source_test: String,
    config: Value,
    input: GoldenInput,
    expected: GoldenExpected,
}

#[derive(Debug, Deserialize)]
struct GoldenInput {
    sequence: u64,
    source_text: String,
    source_lang: String,
    relevant_until_ms: u64,
    wait_for_result_ms: u64,
}

#[derive(Debug, Deserialize)]
struct GoldenExpected {
    published_events: usize,
    stale_results_dropped_min: u64,
}

#[tokio::test]
async fn golden_drops_stale_translation() {
    let path = golden_path();
    assert!(path.is_file(), "missing fixture {path:?}");
    let fixture: GoldenFixture =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("parse");

    let relevant = Arc::new(Mutex::new(HashSet::from([fixture.input.sequence])));
    let config = fixture.config.clone();
    let config_getter: ConfigGetter = Arc::new(move || config.clone());
    let relevant_cb = relevant.clone();
    let relevance = arc_relevance(move |sequence| {
        let relevant = relevant_cb.clone();
        async move { relevant.lock().unwrap().contains(&sequence) }
    });

    let events = Arc::new(Mutex::new(Vec::new()));
    let events_cb = events.clone();
    let publish = arc_publish(move |event| {
        let events = events_cb.clone();
        async move {
            events.lock().unwrap().push(event);
        }
    });

    let engine = TranslationEngine::new_stub(reqwest::Client::new());
    let dispatcher = TranslationDispatcher::new(engine, config_getter, publish, relevance);
    dispatcher.start().await;
    dispatcher
        .submit_final(
            fixture.input.sequence,
            &fixture.input.source_text,
            &fixture.input.source_lang,
            None,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(fixture.input.relevant_until_ms)).await;
    relevant.lock().unwrap().clear();
    tokio::time::sleep(std::time::Duration::from_millis(
        fixture.input.wait_for_result_ms,
    ))
    .await;
    dispatcher.stop().await;

    assert_eq!(
        events.lock().unwrap().len(),
        fixture.expected.published_events,
        "{}",
        fixture.source_test
    );
    assert!(
        dispatcher
            .metrics_snapshot()
            .get("translation_stale_results_dropped")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
            >= fixture.expected.stale_results_dropped_min,
        "{}",
        fixture.source_test
    );
}
