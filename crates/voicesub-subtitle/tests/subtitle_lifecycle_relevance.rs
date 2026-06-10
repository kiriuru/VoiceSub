use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use voicesub_subtitle::{
    ConfigGetter, PublishCallback, SubtitlePayloadEvent, SubtitleRouter, TranscriptEvent,
    TranscriptKind, TranscriptSegment, TranslationEvent, TranslationItem,
};

async fn apply_translation(router: &SubtitleRouter, event: TranslationEvent) {
    router.handle_translation(event).await;
    router.flush_overlay_publish().await;
}

async fn apply_transcript(router: &SubtitleRouter, event: TranscriptEvent) {
    router.handle_transcript(event).await;
    router.flush_overlay_publish().await;
}

fn relevance_config() -> serde_json::Value {
    json!({
        "source_lang": "ru",
        "translation": {
            "enabled": true,
            "target_languages": ["en", "de"],
            "lines": [
                {
                    "slot_id": "translation_1",
                    "enabled": true,
                    "target_lang": "en",
                    "provider": "google_translate_v2",
                    "label": "EN"
                },
                {
                    "slot_id": "translation_2",
                    "enabled": true,
                    "target_lang": "de",
                    "provider": "google_translate_v2",
                    "label": "DE"
                }
            ]
        },
        "subtitle_output": {
            "show_source": true,
            "show_translations": true,
            "max_translation_languages": 2,
            "display_order": ["source", "translation_1", "translation_2"]
        },
        "overlay": { "preset": "stacked", "compact": false },
        "subtitle_style": {},
        "subtitle_lifecycle": {
            "completed_block_ttl_ms": 1200,
            "completed_source_ttl_ms": 200,
            "completed_translation_ttl_ms": 900,
            "pause_to_finalize_ms": 700,
            "allow_early_replace_on_next_final": true,
            "sync_source_and_translation_expiry": false,
            "hard_max_phrase_ms": 12000
        }
    })
}

struct NoopPublisher;

impl NoopPublisher {
    fn callback() -> PublishCallback {
        Arc::new(|_payload| {})
    }
}

fn final_event(sequence: u64, text: &str) -> TranscriptEvent {
    TranscriptEvent {
        event: TranscriptKind::Final,
        text: text.into(),
        sequence,
        segment: Some(TranscriptSegment {
            segment_id: format!("seg-{sequence}"),
            text: text.into(),
            is_final: true,
            source_lang: "ru".into(),
            provider: Some("browser_google".into()),
            sequence,
            revision: 0,
            start_ms: None,
            end_ms: None,
        }),
    }
}

#[tokio::test]
async fn translation_relevance_keeps_still_visible_completed_translation() {
    let config = relevance_config();
    let config_getter: ConfigGetter = Arc::new(move || config.clone());
    let router = SubtitleRouter::new(config_getter, NoopPublisher::callback(), None);

    apply_transcript(&router, final_event(1, "Привет")).await;
    apply_translation(
        &router,
        TranslationEvent {
            sequence: 1,
            source_text: "Привет".into(),
            source_lang: "ru".into(),
            provider: "google_translate_v2".into(),
            is_complete: false,
            translations: vec![TranslationItem {
                slot_id: Some("translation_1".into()),
                label: Some("EN".into()),
                target_lang: "en".into(),
                text: "Hello".into(),
                provider: "google_translate_v2".into(),
                success: true,
                error: None,
                cached: false,
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await;

    tokio::time::sleep(Duration::from_millis(600)).await;

    assert!(router.is_sequence_relevant_for_translation(1).await);
    assert!(router.is_sequence_relevant_for_presentation(1).await);

    apply_translation(
        &router,
        TranslationEvent {
            sequence: 1,
            source_text: "Привет".into(),
            source_lang: "ru".into(),
            provider: "google_translate_v2".into(),
            is_complete: true,
            translations: vec![TranslationItem {
                slot_id: Some("translation_2".into()),
                label: Some("DE".into()),
                target_lang: "de".into(),
                text: "Hallo".into(),
                provider: "google_translate_v2".into(),
                success: true,
                error: None,
                cached: false,
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await;
}

#[tokio::test]
async fn pending_final_promotes_after_previous_stops_awaiting_translation() {
    let config = relevance_config();
    let config_getter: ConfigGetter = Arc::new(move || config.clone());
    let last_payload: Arc<Mutex<Option<SubtitlePayloadEvent>>> = Arc::new(Mutex::new(None));
    let capture = {
        let last_payload = last_payload.clone();
        Arc::new(move |payload: SubtitlePayloadEvent| {
            *last_payload.lock().expect("payload lock") = Some(payload);
        }) as PublishCallback
    };
    let router = SubtitleRouter::new(config_getter, capture, None);

    apply_transcript(&router, final_event(1, "первая фраза")).await;
    apply_translation(
        &router,
        TranslationEvent {
            sequence: 1,
            source_text: "первая фраза".into(),
            source_lang: "ru".into(),
            provider: "google_translate_v2".into(),
            is_complete: false,
            translations: vec![TranslationItem {
                slot_id: Some("translation_1".into()),
                label: Some("EN".into()),
                target_lang: "en".into(),
                text: "first phrase".into(),
                provider: "google_translate_v2".into(),
                success: true,
                error: None,
                cached: false,
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await;

    apply_transcript(&router, final_event(2, "вторая фраза")).await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    let payload = last_payload
        .lock()
        .expect("payload lock")
        .clone()
        .expect("expected promoted subtitle payload");
    assert_eq!(payload.sequence, 2);
    assert_eq!(payload.line1, "вторая фраза");
}

#[tokio::test]
async fn latest_final_stays_translation_relevant_after_source_ttl() {
    let config = relevance_config();
    let config_getter: ConfigGetter = Arc::new(move || config.clone());
    let router = SubtitleRouter::new(config_getter, NoopPublisher::callback(), None);

    apply_transcript(&router, final_event(1, "Поздний перевод")).await;
    tokio::time::sleep(Duration::from_millis(350)).await;

    assert!(router.is_sequence_relevant_for_translation(1).await);
    assert!(router.is_sequence_relevant_for_presentation(1).await);
}
