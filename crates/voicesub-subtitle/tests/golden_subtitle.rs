use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
struct GoldenScenario {
    source_test: String,
    config: Value,
    steps: Vec<GoldenStep>,
    expected: GoldenExpected,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum GoldenStep {
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
        translations: Vec<GoldenTranslationItem>,
        #[serde(default = "default_true")]
        is_complete: bool,
    },
}

#[derive(Debug, Deserialize)]
struct GoldenTranslationItem {
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
struct GoldenExpected {
    lifecycle_state: String,
    #[serde(default)]
    completed_block_visible: bool,
    #[serde(default)]
    active_partial_text: String,
    visible_texts: Vec<String>,
    #[serde(default)]
    translation_slot_id: Option<String>,
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

async fn run_scenario(scenario: GoldenScenario) {
    let config = scenario.config;
    let config_getter: ConfigGetter = Arc::new(move || config.clone());
    let (recorder, publish) = RecordingPublisher::new();
    let router = SubtitleRouter::new(config_getter, publish, None);

    for step in scenario.steps {
        match step {
            GoldenStep::TranscriptFinal { sequence, text } => {
                router
                    .handle_transcript(TranscriptEvent {
                        event: TranscriptKind::Final,
                        text: text.clone(),
                        sequence,
                        segment: Some(TranscriptSegment {
                            segment_id: format!("seg-{sequence}"),
                            text,
                            is_final: true,
                            source_lang: "ru".into(),
                            provider: Some("browser_google".into()),
                            sequence,
                            revision: 0,
                            start_ms: None,
                            end_ms: None,
                        }),
                    })
                    .await;
                router.flush_overlay_publish().await;
            }
            GoldenStep::TranscriptPartial { sequence, text } => {
                router
                    .handle_transcript(TranscriptEvent {
                        event: TranscriptKind::Partial,
                        text: text.clone(),
                        sequence,
                        segment: Some(TranscriptSegment {
                            segment_id: format!("seg-{sequence}"),
                            text,
                            is_final: false,
                            source_lang: "ru".into(),
                            provider: Some("browser_google".into()),
                            sequence,
                            revision: 0,
                            start_ms: None,
                            end_ms: None,
                        }),
                    })
                    .await;
                router.flush_overlay_publish().await;
            }
            GoldenStep::Translation {
                sequence,
                source_text,
                translations,
                is_complete,
            } => {
                router
                    .handle_translation(TranslationEvent {
                        sequence,
                        source_text,
                        source_lang: "ru".into(),
                        provider: "google_translate_v2".into(),
                        is_complete,
                        translations: translations
                            .into_iter()
                            .map(|item| TranslationItem {
                                slot_id: item.slot_id,
                                label: item.label,
                                target_lang: item.target_lang,
                                text: item.text,
                                provider: item.provider,
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
        }
    }

    let payload = recorder.last();
    assert_eq!(
        payload.lifecycle_state,
        lifecycle_from_str(&scenario.expected.lifecycle_state),
        "fixture {} lifecycle_state mismatch",
        scenario.source_test
    );
    if scenario.expected.completed_block_visible {
        assert!(payload.completed_block_visible, "{}", scenario.source_test);
    }
    assert_eq!(
        payload.active_partial_text, scenario.expected.active_partial_text,
        "{}",
        scenario.source_test
    );
    let visible_texts = payload
        .visible_items
        .iter()
        .map(|item| item.text.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        visible_texts,
        scenario
            .expected
            .visible_texts
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        "{}",
        scenario.source_test
    );
    if let Some(slot_id) = scenario.expected.translation_slot_id {
        let translation = payload
            .visible_items
            .iter()
            .find(|item| item.kind == "translation")
            .expect("translation item");
        assert_eq!(translation.slot_id.as_deref(), Some(slot_id.as_str()));
    }
}

fn load_fixture(name: &str) -> GoldenScenario {
    let path = golden_dir().join(name);
    assert!(
        path.is_file(),
        "missing golden fixture {path:?}; add under tests/golden/subtitle/"
    );
    let raw = std::fs::read_to_string(path).expect("read fixture");
    serde_json::from_str(&raw).expect("parse fixture")
}

#[tokio::test]
async fn golden_translation_enriches_completed() {
    run_scenario(load_fixture("translation_enriches_completed.json")).await;
}

#[tokio::test]
async fn golden_completed_with_partial() {
    run_scenario(load_fixture("completed_with_partial.json")).await;
}
