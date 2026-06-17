use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use voicesub_subtitle::{
    SubtitlePayloadEvent, SubtitleRouter, TranscriptEvent, TranscriptKind, TranscriptSegment,
    TranslationEvent, TranslationItem,
};

async fn apply_translation(router: &SubtitleRouter, event: TranslationEvent) {
    router.handle_translation(event).await;
    router.flush_overlay_publish().await;
}

async fn apply_transcript(router: &SubtitleRouter, event: TranscriptEvent) {
    router.handle_transcript(event).await;
    router.flush_overlay_publish().await;
}

fn base_config() -> Value {
    json!({
        "source_lang": "ru",
        "translation": {
            "enabled": true,
            "target_languages": ["en"],
            "lines": [{
                "slot_id": "translation_1",
                "enabled": true,
                "target_lang": "en",
                "provider": "google_translate_v2",
                "label": "EN"
            }]
        },
        "subtitle_output": {
            "show_source": true,
            "show_translations": true,
            "max_translation_languages": 1,
            "display_order": ["source", "translation_1"]
        },
        "overlay": { "preset": "stacked", "compact": false },
        "subtitle_style": {},
        "subtitle_lifecycle": {
            "completed_block_ttl_ms": 10_000,
            "completed_source_ttl_ms": 10_000,
            "completed_translation_ttl_ms": 10_000,
            "pause_to_finalize_ms": 700,
            "allow_early_replace_on_next_final": true,
            "sync_source_and_translation_expiry": true,
            "hard_max_phrase_ms": 12_000
        }
    })
}

struct RecordingPublisher {
    messages: Arc<Mutex<Vec<SubtitlePayloadEvent>>>,
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

    fn last(&self) -> SubtitlePayloadEvent {
        self.messages
            .lock()
            .unwrap()
            .last()
            .cloned()
            .expect("expected published payload")
    }
}

use voicesub_subtitle::PublishCallback;

#[tokio::test]
async fn translation_update_enriches_completed_block() {
    let config = base_config();
    let config_getter: voicesub_subtitle::ConfigGetter = Arc::new(move || config.clone());
    let (recorder, publish) = RecordingPublisher::new();
    let router = SubtitleRouter::new(config_getter, publish, None);

    apply_transcript(
        &router,
        TranscriptEvent {
            event: TranscriptKind::Final,
            text: "Привет".into(),
            sequence: 1,
            segment: Some(TranscriptSegment {
                segment_id: "seg-1".into(),
                text: "Привет".into(),
                is_final: true,
                source_lang: "ru".into(),
                provider: Some("browser_google".into()),
                sequence: 1,
                revision: 0,
                start_ms: None,
                end_ms: None,
            }),
        },
    )
    .await;

    let before = recorder.last();
    assert_eq!(
        before.lifecycle_state,
        voicesub_subtitle::LifecycleState::CompletedOnly
    );
    assert!(before.completed_block_visible);
    assert_eq!(before.active_partial_text, "");
    assert_eq!(
        before
            .visible_items
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Привет"]
    );

    apply_translation(
        &router,
        TranslationEvent {
            sequence: 1,
            source_text: "Привет".into(),
            source_lang: "ru".into(),
            provider: "google_translate_v2".into(),
            is_complete: true,
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

    let after = recorder.last();
    assert_eq!(
        after.lifecycle_state,
        voicesub_subtitle::LifecycleState::CompletedOnly
    );
    assert!(after.completed_block_visible);
    assert_eq!(after.active_partial_text, "");
    assert_eq!(
        after
            .visible_items
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Привет", "Hello"]
    );
    assert_eq!(
        after.visible_items[1].slot_id.as_deref(),
        Some("translation_1")
    );
    assert_eq!(
        after.visible_items[1].style_slot.as_deref(),
        Some("translation_1")
    );
}

#[tokio::test]
async fn reset_clears_records() {
    let config = base_config();
    let config_getter: voicesub_subtitle::ConfigGetter = Arc::new(move || config.clone());
    let (recorder, publish) = RecordingPublisher::new();
    let router = SubtitleRouter::new(config_getter, publish, None);

    apply_transcript(
        &router,
        TranscriptEvent {
            event: TranscriptKind::Final,
            text: "Привет".into(),
            sequence: 1,
            segment: None,
        },
    )
    .await;
    assert!(!recorder.messages.lock().unwrap().is_empty());

    router.reset().await;
    let payload = recorder.last();
    assert_eq!(
        payload.lifecycle_state,
        voicesub_subtitle::LifecycleState::Idle
    );
    assert_eq!(payload.active_partial_text, "");
    assert!(payload.visible_items.is_empty());
}

#[tokio::test]
async fn partial_event_publishes_active_partial() {
    let config = base_config();
    let config_getter: voicesub_subtitle::ConfigGetter = Arc::new(move || config.clone());
    let (recorder, publish) = RecordingPublisher::new();
    let router = SubtitleRouter::new(config_getter, publish, None);

    apply_transcript(
        &router,
        TranscriptEvent {
            event: TranscriptKind::Partial,
            text: "Прив".into(),
            sequence: 1,
            segment: Some(TranscriptSegment {
                segment_id: "seg-partial".into(),
                text: "Прив".into(),
                is_final: false,
                source_lang: "ru".into(),
                provider: Some("browser_google".into()),
                sequence: 1,
                revision: 0,
                start_ms: None,
                end_ms: None,
            }),
        },
    )
    .await;

    let payload = recorder.last();
    assert_eq!(
        payload.lifecycle_state,
        voicesub_subtitle::LifecycleState::PartialOnly
    );
    assert_eq!(payload.active_partial_text, "Прив");
    assert_eq!(
        payload
            .visible_items
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Прив"]
    );
}

fn final_transcript(sequence: u64, text: &str) -> TranscriptEvent {
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

fn partial_transcript(sequence: u64, text: &str) -> TranscriptEvent {
    TranscriptEvent {
        event: TranscriptKind::Partial,
        text: text.into(),
        sequence,
        segment: Some(TranscriptSegment {
            segment_id: format!("seg-{sequence}"),
            text: text.into(),
            is_final: false,
            source_lang: "ru".into(),
            provider: Some("browser_google".into()),
            sequence,
            revision: 0,
            start_ms: None,
            end_ms: None,
        }),
    }
}

fn router_with_config(config: Value) -> (RecordingPublisher, Arc<SubtitleRouter>) {
    let config_getter: voicesub_subtitle::ConfigGetter = Arc::new(move || config.clone());
    let (recorder, publish) = RecordingPublisher::new();
    let router = SubtitleRouter::new(config_getter, publish, None);
    (recorder, router)
}

#[tokio::test]
async fn stale_translation_for_old_sequence_is_not_presentation_relevant() {
    let (recorder, router) = router_with_config(base_config());
    apply_transcript(&router, final_transcript(2, "Новый")).await;
    apply_translation(
        &router,
        TranslationEvent {
            sequence: 1,
            source_text: "Старый".into(),
            source_lang: "ru".into(),
            provider: "google_translate_v2".into(),
            is_complete: false,
            translations: vec![TranslationItem {
                slot_id: Some("translation_1".into()),
                label: Some("EN".into()),
                target_lang: "en".into(),
                text: "Old translation".into(),
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

    assert!(!router.is_sequence_relevant_for_presentation(1).await);
    let payload = recorder.last();
    assert_eq!(
        payload
            .visible_items
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Новый"]
    );
}

#[tokio::test]
async fn legacy_language_display_order_maps_to_slot_ids() {
    let mut config = base_config();
    config["subtitle_output"]["display_order"] = json!(["source", "en"]);
    let (recorder, router) = router_with_config(config);

    apply_transcript(&router, final_transcript(1, "Привет")).await;
    apply_translation(
        &router,
        TranslationEvent {
            sequence: 1,
            source_text: "Привет".into(),
            source_lang: "ru".into(),
            provider: "google_translate_v2".into(),
            is_complete: true,
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

    let payload = recorder.last();
    assert_eq!(payload.display_order, vec!["source", "translation_1"]);
    assert_eq!(
        payload
            .visible_items
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Привет", "Hello"]
    );
}

#[tokio::test]
async fn translation_without_slot_id_is_mapped_using_target_language() {
    let (recorder, router) = router_with_config(base_config());
    apply_transcript(&router, final_transcript(1, "Привет")).await;
    apply_translation(
        &router,
        TranslationEvent {
            sequence: 1,
            source_text: "Привет".into(),
            source_lang: "ru".into(),
            provider: "google_translate_v2".into(),
            is_complete: true,
            translations: vec![TranslationItem {
                slot_id: None,
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

    let payload = recorder.last();
    assert_eq!(
        payload
            .visible_items
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Привет", "Hello"]
    );
    assert_eq!(
        payload.visible_items[1].slot_id.as_deref(),
        Some("translation_1")
    );
    assert_eq!(
        payload.visible_items[1].style_slot.as_deref(),
        Some("translation_1")
    );
}

#[tokio::test]
async fn new_partial_keeps_previous_completed_translation_block_visible() {
    let (recorder, router) = router_with_config(base_config());
    apply_transcript(&router, final_transcript(1, "Первая фраза")).await;
    apply_translation(
        &router,
        TranslationEvent {
            sequence: 1,
            source_text: "Первая фраза".into(),
            source_lang: "ru".into(),
            provider: "google_translate_v2".into(),
            is_complete: true,
            translations: vec![TranslationItem {
                slot_id: Some("translation_1".into()),
                label: Some("EN".into()),
                target_lang: "en".into(),
                text: "First phrase".into(),
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
    apply_transcript(&router, partial_transcript(2, "Новая")).await;

    let payload = recorder.last();
    assert_eq!(
        payload.lifecycle_state,
        voicesub_subtitle::LifecycleState::CompletedWithPartial
    );
    assert!(payload.completed_block_visible);
    assert_eq!(payload.active_partial_text, "Новая");
    assert_eq!(
        payload
            .visible_items
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Новая", "First phrase"]
    );
}

#[tokio::test]
async fn browser_same_sequence_partial_keeps_completed_translations() {
    let mut config = base_config();
    config["translation"]["lines"] = json!([
        {
            "slot_id": "translation_2",
            "enabled": true,
            "target_lang": "en",
            "provider": "google_web",
            "label": "EN"
        },
        {
            "slot_id": "translation_1",
            "enabled": true,
            "target_lang": "ja",
            "provider": "google_web",
            "label": "JA"
        }
    ]);
    config["subtitle_output"]["max_translation_languages"] = json!(2);
    config["subtitle_output"]["display_order"] =
        json!(["source", "translation_2", "translation_1"]);
    let (recorder, router) = router_with_config(config);

    router
        .ingest_browser_text(
            "Добрый вечер дамы и господа",
            true,
            Some("ru"),
            Some("browser_google"),
        )
        .await;
    apply_translation(
        &router,
        TranslationEvent {
            sequence: 1,
            source_text: "Добрый вечер дамы и господа".into(),
            source_lang: "ru".into(),
            provider: "google_web".into(),
            is_complete: true,
            translations: vec![
                TranslationItem {
                    slot_id: Some("translation_2".into()),
                    label: Some("EN".into()),
                    target_lang: "en".into(),
                    text: "Good evening ladies and gentlemen".into(),
                    provider: "google_web".into(),
                    success: true,
                    error: None,
                    cached: false,
                    ..Default::default()
                },
                TranslationItem {
                    slot_id: Some("translation_1".into()),
                    label: Some("JA".into()),
                    target_lang: "ja".into(),
                    text: "皆さん、こんばんは".into(),
                    provider: "google_web".into(),
                    success: true,
                    error: None,
                    cached: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
    )
    .await;
    router
        .ingest_browser_text("а", false, Some("ru"), Some("browser_google"))
        .await;

    let payload = recorder.last();
    assert_eq!(
        payload.lifecycle_state,
        voicesub_subtitle::LifecycleState::CompletedWithPartial,
        "lifecycle_state"
    );
    let texts: Vec<&str> = payload
        .visible_items
        .iter()
        .map(|item| item.text.as_str())
        .collect();
    assert!(
        texts.contains(&"Good evening ladies and gentlemen"),
        "expected EN translation in visible_items, got {:?}",
        texts
    );
    assert_eq!(
        payload.active_partial_sequence,
        Some(2),
        "browser partial after final must advance sequence like SST"
    );
}

#[tokio::test]
async fn browser_partial_after_final_advances_sequence() {
    let (recorder, router) = router_with_config(base_config());

    let final_sequence = router
        .ingest_browser_text("hello", true, Some("en"), Some("browser_google"))
        .await;
    let partial_sequence = router
        .ingest_browser_text("hello world", false, Some("en"), Some("browser_google"))
        .await;

    assert_eq!(final_sequence, 1);
    assert_eq!(partial_sequence, 2);
    assert_eq!(recorder.last().active_partial_sequence, Some(2));
}

#[tokio::test]
async fn partial_only_payload_includes_source_when_translations_hidden() {
    let mut config = base_config();
    config["subtitle_output"]["show_translations"] = json!(false);
    let (recorder, router) = router_with_config(config);

    apply_transcript(&router, final_transcript(1, "Привет")).await;
    apply_transcript(&router, partial_transcript(2, "yes")).await;

    let payload = recorder.last();
    assert_eq!(
        payload.lifecycle_state,
        voicesub_subtitle::LifecycleState::PartialOnly
    );
    assert_eq!(payload.active_partial_text, "yes");
    assert_eq!(payload.line1, "yes");
    assert_eq!(payload.visible_items.len(), 1);
    assert_eq!(payload.visible_items[0].kind, "source");
    assert_eq!(payload.visible_items[0].text, "yes");
}

#[tokio::test]
async fn partial_only_payload_hides_source_when_show_source_false() {
    let mut config = base_config();
    config["subtitle_output"]["show_source"] = json!(false);
    config["subtitle_output"]["show_translations"] = json!(false);
    let (recorder, router) = router_with_config(config);

    apply_transcript(&router, final_transcript(1, "Привет")).await;
    apply_transcript(&router, partial_transcript(2, "yes")).await;

    let payload = recorder.last();
    assert_eq!(
        payload.lifecycle_state,
        voicesub_subtitle::LifecycleState::PartialOnly
    );
    assert_eq!(payload.active_partial_text, "");
    assert_eq!(payload.line1, "");
    assert!(payload.visible_items.is_empty());
}

#[tokio::test]
async fn duplicate_target_languages_render_separate_translation_slots() {
    let mut config = base_config();
    config["translation"]["lines"] = json!([
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
    ]);
    config["subtitle_output"]["max_translation_languages"] = json!(2);
    config["subtitle_output"]["display_order"] =
        json!(["source", "translation_2", "translation_1"]);
    let (recorder, router) = router_with_config(config);

    apply_transcript(&router, final_transcript(4, "Два перевода")).await;
    apply_translation(
        &router,
        TranslationEvent {
            sequence: 4,
            source_text: "Два перевода".into(),
            source_lang: "ru".into(),
            provider: "mixed".into(),
            is_complete: true,
            translations: vec![
                TranslationItem {
                    slot_id: Some("translation_1".into()),
                    label: Some("EN-G".into()),
                    target_lang: "en".into(),
                    text: "Google hello".into(),
                    provider: "google_translate_v2".into(),
                    success: true,
                    error: None,
                    cached: false,
                    ..Default::default()
                },
                TranslationItem {
                    slot_id: Some("translation_2".into()),
                    label: Some("EN-AI".into()),
                    target_lang: "en".into(),
                    text: "OpenAI hello".into(),
                    provider: "openai".into(),
                    success: true,
                    error: None,
                    cached: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
    )
    .await;

    let payload = recorder.last();
    assert_eq!(
        payload.display_order,
        vec!["source", "translation_2", "translation_1"]
    );
    assert_eq!(
        payload
            .visible_items
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Два перевода", "OpenAI hello", "Google hello"]
    );
    let translation_slots = payload
        .visible_items
        .iter()
        .filter(|item| item.kind == "translation")
        .map(|item| item.slot_id.as_deref().unwrap_or(""))
        .collect::<Vec<_>>();
    assert_eq!(translation_slots, vec!["translation_2", "translation_1"]);
}

async fn wait_for_visible_texts(
    recorder: &RecordingPublisher,
    expected: &[&str],
    timeout: Duration,
) -> SubtitlePayloadEvent {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let payload = recorder.last();
        let texts: Vec<String> = payload
            .visible_items
            .iter()
            .map(|item| item.text.clone())
            .collect();
        let matches = texts.len() == expected.len()
            && expected
                .iter()
                .all(|text| texts.iter().any(|visible| visible == *text));
        if matches || tokio::time::Instant::now() >= deadline {
            return payload;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn completed_source_expires_before_translation_then_returns_to_idle() {
    let mut config = base_config();
    config["subtitle_lifecycle"]["completed_source_ttl_ms"] = json!(500);
    config["subtitle_lifecycle"]["completed_translation_ttl_ms"] = json!(900);
    config["subtitle_lifecycle"]["sync_source_and_translation_expiry"] = json!(false);
    let (recorder, router) = router_with_config(config);

    apply_transcript(&router, final_transcript(1, "Первая фраза")).await;
    apply_translation(
        &router,
        TranslationEvent {
            sequence: 1,
            source_text: "Первая фраза".into(),
            source_lang: "ru".into(),
            provider: "google_translate_v2".into(),
            is_complete: false,
            translations: vec![TranslationItem {
                slot_id: Some("translation_1".into()),
                label: Some("EN".into()),
                target_lang: "en".into(),
                text: "First phrase".into(),
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

    let after_source_ttl =
        wait_for_visible_texts(&recorder, &["First phrase"], Duration::from_secs(2)).await;
    assert_eq!(
        after_source_ttl.lifecycle_state,
        voicesub_subtitle::LifecycleState::CompletedOnly
    );

    let after_all_ttl = wait_for_visible_texts(&recorder, &[], Duration::from_secs(2)).await;
    assert_eq!(
        after_all_ttl.lifecycle_state,
        voicesub_subtitle::LifecycleState::Idle
    );
    assert!(!after_all_ttl.completed_block_visible);
}

#[tokio::test]
async fn late_translation_after_source_ttl_reappears_as_translation_only() {
    let mut config = base_config();
    config["subtitle_lifecycle"]["completed_source_ttl_ms"] = json!(500);
    config["subtitle_lifecycle"]["completed_translation_ttl_ms"] = json!(1400);
    config["subtitle_lifecycle"]["sync_source_and_translation_expiry"] = json!(false);
    let (recorder, router) = router_with_config(config);

    apply_transcript(&router, final_transcript(1, "Поздний перевод")).await;

    let after_source_ttl = wait_for_visible_texts(&recorder, &[], Duration::from_secs(2)).await;
    assert_eq!(
        after_source_ttl.lifecycle_state,
        voicesub_subtitle::LifecycleState::Idle
    );

    apply_translation(
        &router,
        TranslationEvent {
            sequence: 1,
            source_text: "Поздний перевод".into(),
            source_lang: "ru".into(),
            provider: "google_translate_v2".into(),
            is_complete: false,
            translations: vec![TranslationItem {
                slot_id: Some("translation_1".into()),
                label: Some("EN".into()),
                target_lang: "en".into(),
                text: "Late translation".into(),
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

    let after_translation =
        wait_for_visible_texts(&recorder, &["Late translation"], Duration::from_millis(200)).await;
    assert_eq!(
        after_translation.lifecycle_state,
        voicesub_subtitle::LifecycleState::CompletedOnly
    );
    assert!(after_translation.completed_block_visible);
}

#[tokio::test]
async fn sync_expiry_keeps_source_until_translation_ttl() {
    let mut config = base_config();
    config["subtitle_lifecycle"]["completed_source_ttl_ms"] = json!(500);
    config["subtitle_lifecycle"]["completed_translation_ttl_ms"] = json!(1800);
    config["subtitle_lifecycle"]["sync_source_and_translation_expiry"] = json!(true);
    let (recorder, router) = router_with_config(config);

    apply_transcript(&router, final_transcript(1, "Синхронный TTL")).await;
    apply_translation(
        &router,
        TranslationEvent {
            sequence: 1,
            source_text: "Синхронный TTL".into(),
            source_lang: "ru".into(),
            provider: "google_translate_v2".into(),
            is_complete: true,
            translations: vec![TranslationItem {
                slot_id: Some("translation_1".into()),
                label: Some("EN".into()),
                target_lang: "en".into(),
                text: "Synced TTL".into(),
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

    let mid_window = wait_for_visible_texts(
        &recorder,
        &["Синхронный TTL", "Synced TTL"],
        Duration::from_millis(900),
    )
    .await;
    assert_eq!(
        mid_window.lifecycle_state,
        voicesub_subtitle::LifecycleState::CompletedOnly
    );

    let after_translation_ttl =
        wait_for_visible_texts(&recorder, &[], Duration::from_secs(2)).await;
    assert_eq!(
        after_translation_ttl.lifecycle_state,
        voicesub_subtitle::LifecycleState::Idle
    );
}

#[tokio::test]
async fn independent_ttl_uses_configured_hold_seconds() {
    let mut config = base_config();
    config["subtitle_lifecycle"]["completed_source_ttl_ms"] = json!(800);
    config["subtitle_lifecycle"]["completed_translation_ttl_ms"] = json!(1600);
    config["subtitle_lifecycle"]["sync_source_and_translation_expiry"] = json!(false);
    let (recorder, router) = router_with_config(config);

    apply_transcript(&router, final_transcript(1, "Независимый TTL")).await;
    apply_translation(
        &router,
        TranslationEvent {
            sequence: 1,
            source_text: "Независимый TTL".into(),
            source_lang: "ru".into(),
            provider: "google_translate_v2".into(),
            is_complete: true,
            translations: vec![TranslationItem {
                slot_id: Some("translation_1".into()),
                label: Some("EN".into()),
                target_lang: "en".into(),
                text: "Independent TTL".into(),
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

    let after_source_only =
        wait_for_visible_texts(&recorder, &["Independent TTL"], Duration::from_secs(2)).await;
    assert_eq!(
        after_source_only.lifecycle_state,
        voicesub_subtitle::LifecycleState::CompletedOnly
    );
    assert!(
        !after_source_only
            .visible_items
            .iter()
            .any(|item| item.kind == "source"),
        "source should expire before translation when sync is disabled"
    );

    let after_translation_only =
        wait_for_visible_texts(&recorder, &[], Duration::from_secs(2)).await;
    assert_eq!(
        after_translation_only.lifecycle_state,
        voicesub_subtitle::LifecycleState::Idle
    );
}

#[tokio::test]
async fn failed_target_counts_as_received_for_lifecycle() {
    let (recorder, router) = router_with_config(base_config());

    apply_transcript(&router, final_transcript(3, "Ошибка перевода")).await;
    apply_translation(
        &router,
        TranslationEvent {
            sequence: 3,
            source_text: "Ошибка перевода".into(),
            source_lang: "ru".into(),
            provider: "google_translate_v2".into(),
            is_complete: false,
            translations: vec![TranslationItem {
                slot_id: Some("translation_1".into()),
                label: Some("EN".into()),
                target_lang: "en".into(),
                text: String::new(),
                provider: "google_translate_v2".into(),
                success: false,
                error: Some("timeout".into()),
                cached: false,
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await;

    let record = router.record_for_sequence(3).await.expect("record");
    assert_eq!(
        record
            .get("translation_received")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    let payload = recorder.last();
    assert_eq!(
        payload.lifecycle_state,
        voicesub_subtitle::LifecycleState::CompletedOnly
    );
    assert_eq!(
        payload
            .visible_items
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Ошибка перевода"]
    );
}

#[test]
fn overlay_publish_adds_created_at_ms() {
    use std::sync::Mutex;

    use voicesub_subtitle::{LifecycleState, OverlayBroadcaster, SubtitlePayloadEvent};

    let messages = Arc::new(Mutex::new(Vec::new()));
    let messages_cb = messages.clone();
    let broadcaster = OverlayBroadcaster::new(
        Arc::new(move |message| {
            messages_cb.lock().unwrap().push(message);
        }),
        voicesub_subtitle::SubtitleLog::default(),
    );
    let payload = SubtitlePayloadEvent {
        lifecycle_state: LifecycleState::PartialOnly,
        active_partial_text: "Превью".into(),
        ..Default::default()
    };
    assert!(broadcaster.publish(&payload));

    let message = messages
        .lock()
        .unwrap()
        .first()
        .cloned()
        .expect("overlay frame");
    let created_at_ms = message
        .get("payload")
        .and_then(|value| value.get("created_at_ms"))
        .and_then(|value| value.as_u64())
        .expect("created_at_ms should be present");
    assert!(created_at_ms > 0);
}
