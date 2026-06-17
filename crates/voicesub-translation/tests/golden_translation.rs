use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::Value;
use voicesub_subtitle::TranslationEvent;
use voicesub_translation::{
    ConfigGetter, TranslationDispatcher, TranslationEngine, arc_publish, arc_relevance,
};

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("tests/golden/translation/publishes_fresh_translation.json")
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
    relevant_sequences: Vec<u64>,
}

#[derive(Debug, Deserialize)]
struct GoldenExpected {
    incremental_count: usize,
    incremental_text: String,
    incremental_slot_id: String,
    complete_count: usize,
    complete_texts: Vec<String>,
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

#[tokio::test]
async fn golden_publishes_fresh_translation() {
    let path = golden_path();
    assert!(path.is_file(), "missing fixture {path:?}");
    let fixture: GoldenFixture =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("parse");

    let relevant = Arc::new(Mutex::new(
        fixture
            .input
            .relevant_sequences
            .into_iter()
            .collect::<HashSet<_>>(),
    ));
    let config = fixture.config.clone();
    let config_getter: ConfigGetter = Arc::new(move || config.clone());
    let relevant_cb = relevant.clone();
    let relevance = arc_relevance(move |sequence| {
        let relevant = relevant_cb.clone();
        async move { relevant.lock().unwrap().contains(&sequence) }
    });
    let (recorder, publish) = RecordingPublisher::new();

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
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    dispatcher.stop().await;

    let events = recorder.events.lock().unwrap().clone();
    let incremental: Vec<_> = events
        .iter()
        .filter(|event| !event.is_complete && !event.translations.is_empty())
        .collect();
    let complete: Vec<_> = events.iter().filter(|event| event.is_complete).collect();

    assert_eq!(
        incremental.len(),
        fixture.expected.incremental_count,
        "{}",
        fixture.source_test
    );
    assert_eq!(
        incremental[0].translations[0].text, fixture.expected.incremental_text,
        "{}",
        fixture.source_test
    );
    assert_eq!(
        incremental[0].translations[0].slot_id.as_deref(),
        Some(fixture.expected.incremental_slot_id.as_str()),
        "{}",
        fixture.source_test
    );
    assert_eq!(
        complete.len(),
        fixture.expected.complete_count,
        "{}",
        fixture.source_test
    );
    assert_eq!(
        complete[0]
            .translations
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        fixture
            .expected
            .complete_texts
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        "{}",
        fixture.source_test
    );
}
