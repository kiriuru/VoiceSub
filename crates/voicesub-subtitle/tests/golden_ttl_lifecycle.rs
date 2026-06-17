use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use voicesub_subtitle::{
    ConfigGetter, LifecycleState, PublishCallback, SubtitleRouter, TranscriptEvent, TranscriptKind,
    TranscriptSegment, TranslationEvent, TranslationItem,
};

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("tests/golden/subtitle")
}

#[derive(Debug, Deserialize)]
struct TtlGoldenScenario {
    source_test: String,
    config: Value,
    steps: Vec<TtlGoldenStep>,
    checkpoints: Vec<TtlCheckpoint>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TtlGoldenStep {
    TranscriptFinal {
        sequence: u64,
        text: String,
    },
    TranscriptPartial {
        sequence: u64,
        text: String,
    },
    Translation {
        sequence: u64,
        source_text: String,
        translations: Vec<TtlGoldenTranslationItem>,
        #[serde(default = "default_true")]
        is_complete: bool,
    },
    SleepMs {
        ms: u64,
    },
}

#[derive(Debug, Deserialize)]
struct TtlGoldenTranslationItem {
    slot_id: Option<String>,
    label: Option<String>,
    target_lang: String,
    text: String,
    provider: String,
    #[serde(default = "default_true")]
    success: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct TtlCheckpoint {
    after_step: usize,
    expected: TtlExpected,
}

#[derive(Debug, Deserialize)]
struct TtlExpected {
    lifecycle_state: String,
    #[serde(default)]
    completed_block_visible: bool,
    #[serde(default)]
    active_partial_text: String,
    visible_texts: Vec<String>,
}

fn lifecycle_from_str(value: &str) -> LifecycleState {
    match value {
        "partial_only" => LifecycleState::PartialOnly,
        "completed_only" => LifecycleState::CompletedOnly,
        "completed_with_partial" => LifecycleState::CompletedWithPartial,
        _ => LifecycleState::Idle,
    }
}

struct RecordingPublisher {
    messages: Arc<Mutex<Vec<voicesub_subtitle::SubtitlePayloadEvent>>>,
}

impl RecordingPublisher {
    fn new() -> (Self, PublishCallback) {
        let messages = Arc::new(Mutex::new(Vec::new()));
        let messages_cb = messages.clone();
        let publish: PublishCallback = Arc::new(move |payload| {
            messages_cb.lock().unwrap().push(payload);
        });
        (Self { messages }, publish)
    }

    fn last(&self) -> voicesub_subtitle::SubtitlePayloadEvent {
        self.messages
            .lock()
            .unwrap()
            .last()
            .cloned()
            .expect("expected published payload")
    }
}

async fn execute_step(router: &SubtitleRouter, step: &TtlGoldenStep) {
    match step {
        TtlGoldenStep::TranscriptFinal { sequence, text } => {
            router
                .handle_transcript(TranscriptEvent {
                    event: TranscriptKind::Final,
                    text: text.clone(),
                    sequence: *sequence,
                    segment: Some(TranscriptSegment {
                        segment_id: format!("seg-{sequence}"),
                        text: text.clone(),
                        is_final: true,
                        source_lang: "ru".into(),
                        provider: Some("browser_google".into()),
                        sequence: *sequence,
                        revision: 0,
                        start_ms: None,
                        end_ms: None,
                    }),
                })
                .await;
            router.flush_overlay_publish().await;
        }
        TtlGoldenStep::TranscriptPartial { sequence, text } => {
            router
                .handle_transcript(TranscriptEvent {
                    event: TranscriptKind::Partial,
                    text: text.clone(),
                    sequence: *sequence,
                    segment: Some(TranscriptSegment {
                        segment_id: format!("seg-{sequence}"),
                        text: text.clone(),
                        is_final: false,
                        source_lang: "ru".into(),
                        provider: Some("browser_google".into()),
                        sequence: *sequence,
                        revision: 0,
                        start_ms: None,
                        end_ms: None,
                    }),
                })
                .await;
            router.flush_overlay_publish().await;
        }
        TtlGoldenStep::Translation {
            sequence,
            source_text,
            translations,
            is_complete,
        } => {
            router
                .handle_translation(TranslationEvent {
                    sequence: *sequence,
                    source_text: source_text.clone(),
                    source_lang: "ru".into(),
                    provider: "google_translate_v2".into(),
                    is_complete: *is_complete,
                    translations: translations
                        .iter()
                        .map(|item| TranslationItem {
                            slot_id: item.slot_id.clone(),
                            label: item.label.clone(),
                            target_lang: item.target_lang.clone(),
                            text: item.text.clone(),
                            provider: item.provider.clone(),
                            success: item.success,
                            error: None,
                            cached: false,
                            ..Default::default()
                        })
                        .collect(),
                    ..Default::default()
                })
                .await;
            router.flush_overlay_publish().await;
        }
        TtlGoldenStep::SleepMs { ms } => {
            tokio::time::sleep(Duration::from_millis(*ms)).await;
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
    }
}

fn assert_expected(recorder: &RecordingPublisher, expected: &TtlExpected, source_test: &str) {
    let payload = recorder.last();
    assert_eq!(
        payload.lifecycle_state,
        lifecycle_from_str(&expected.lifecycle_state),
        "{source_test} lifecycle_state"
    );
    if expected.completed_block_visible {
        assert!(
            payload.completed_block_visible,
            "{source_test} completed_block"
        );
    }
    assert_eq!(
        payload.active_partial_text, expected.active_partial_text,
        "{source_test} active_partial_text"
    );
    let visible_texts = payload
        .visible_items
        .iter()
        .map(|item| item.text.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        visible_texts,
        expected
            .visible_texts
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        "{source_test} visible_texts"
    );
}

async fn run_ttl_scenario(scenario: TtlGoldenScenario) {
    let config = scenario.config;
    let config_getter: ConfigGetter = Arc::new(move || config.clone());
    let (recorder, publish) = RecordingPublisher::new();
    let router = SubtitleRouter::new(config_getter, publish, None);

    let mut step_idx = 0usize;
    for checkpoint in &scenario.checkpoints {
        while step_idx < checkpoint.after_step {
            execute_step(&router, &scenario.steps[step_idx]).await;
            step_idx += 1;
        }
        assert_expected(&recorder, &checkpoint.expected, &scenario.source_test);
    }
    while step_idx < scenario.steps.len() {
        execute_step(&router, &scenario.steps[step_idx]).await;
        step_idx += 1;
    }
}

fn load_ttl_fixture(name: &str) -> TtlGoldenScenario {
    let path = golden_dir().join(name);
    assert!(path.is_file(), "missing golden fixture {path:?}");
    serde_json::from_str(&std::fs::read_to_string(path).expect("read fixture")).expect("parse")
}

#[tokio::test]
async fn golden_ttl_full_phrase_lifecycle() {
    run_ttl_scenario(load_ttl_fixture("ttl_full_phrase_lifecycle.json")).await;
}
