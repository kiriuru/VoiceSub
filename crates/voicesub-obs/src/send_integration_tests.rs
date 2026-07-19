use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde_json::json;
use voicesub_subtitle::{ConfigGetter, LifecycleState, SubtitleLineItem, SubtitlePayloadEvent};

use crate::ObsCaptionService;
use crate::client::MockObsClient;

fn obs_source_live_config(debug_mirror: bool) -> serde_json::Value {
    json!({
        "obs_closed_captions": {
            "enabled": true,
            "output_mode": "source_live",
            "connection": {
                "host": "127.0.0.1",
                "port": 4455,
                "password": ""
            },
            "debug_mirror": {
                "enabled": debug_mirror,
                "input_name": "CC_DEBUG",
                "send_partials": true
            },
            "timing": {
                "send_partials": true,
                "partial_throttle_ms": 0,
                "min_partial_delta_chars": 1,
                "final_replace_delay_ms": 0,
                "clear_after_ms": 0,
                "avoid_duplicate_text": false
            }
        }
    })
}

fn obs_config(output_mode: &str, debug_mirror: bool) -> serde_json::Value {
    json!({
        "obs_closed_captions": {
            "enabled": true,
            "output_mode": output_mode,
            "connection": {
                "host": "127.0.0.1",
                "port": 4455,
                "password": ""
            },
            "debug_mirror": {
                "enabled": debug_mirror,
                "input_name": "CC_DEBUG",
                "send_partials": true
            },
            "timing": {
                "send_partials": true,
                "partial_throttle_ms": 140,
                "min_partial_delta_chars": 1,
                "final_replace_delay_ms": 0,
                "clear_after_ms": 0,
                "avoid_duplicate_text": true
            }
        }
    })
}

fn obs_base_config(output_mode: &str, debug_mirror: bool) -> serde_json::Value {
    json!({
        "obs_closed_captions": {
            "enabled": true,
            "output_mode": output_mode,
            "connection": {
                "host": "127.0.0.1",
                "port": 4455,
                "password": ""
            },
            "debug_mirror": {
                "enabled": debug_mirror,
                "input_name": "CC_DEBUG",
                "send_partials": true
            },
            "timing": {
                "send_partials": true,
                "partial_throttle_ms": 1000,
                "min_partial_delta_chars": 3,
                "final_replace_delay_ms": 0,
                "clear_after_ms": 2500,
                "avoid_duplicate_text": true
            }
        }
    })
}

fn obs_config_with_timing(
    output_mode: &str,
    debug_mirror: bool,
    timing: serde_json::Value,
) -> serde_json::Value {
    json!({
        "obs_closed_captions": {
            "enabled": true,
            "output_mode": output_mode,
            "connection": {
                "host": "127.0.0.1",
                "port": 4455,
                "password": ""
            },
            "debug_mirror": {
                "enabled": debug_mirror,
                "input_name": "CC_DEBUG",
                "send_partials": true
            },
            "timing": timing
        }
    })
}

fn translation_line_item(text: &str, slot: &str, lang: &str, label: &str) -> SubtitleLineItem {
    SubtitleLineItem {
        kind: "translation".into(),
        lang: lang.into(),
        label: label.into(),
        text: text.into(),
        style_slot: Some(slot.into()),
        slot_id: Some(slot.into()),
        target_lang: Some(lang.into()),
        provider: None,
        visible: true,
        success: true,
        error: None,
    }
}

fn source_line_item(text: &str) -> SubtitleLineItem {
    SubtitleLineItem {
        kind: "source".into(),
        lang: "ru".into(),
        label: "RU".into(),
        text: text.into(),
        style_slot: Some("source".into()),
        slot_id: None,
        target_lang: None,
        provider: None,
        visible: true,
        success: true,
        error: None,
    }
}

fn payload_for_sequence(
    sequence: u64,
    visible_items: Vec<SubtitleLineItem>,
) -> SubtitlePayloadEvent {
    SubtitlePayloadEvent {
        sequence,
        source_lang: "ru".into(),
        source_text: "Привет".into(),
        display_order: vec!["source".into(), "en".into(), "de".into()],
        show_source: true,
        show_translations: true,
        max_translation_languages: 2,
        items: visible_items.clone(),
        visible_items,
        lifecycle_state: LifecycleState::CompletedOnly,
        completed_block_visible: true,
        line1: "Привет".into(),
        line2: "Hello\nHallo".into(),
        ..SubtitlePayloadEvent::default()
    }
}

fn stream_caption_texts(
    requests: &std::sync::Arc<std::sync::Mutex<Vec<(String, serde_json::Value)>>>,
) -> Vec<String> {
    requests
        .lock()
        .unwrap()
        .iter()
        .filter(|(kind, _)| kind == "SendStreamCaption")
        .filter_map(|(_, data)| data.get("captionText")?.as_str().map(str::to_string))
        .collect()
}

fn make_service(config: serde_json::Value) -> Arc<ObsCaptionService> {
    let config = Arc::new(RwLock::new(config));
    let getter: ConfigGetter = Arc::new(move || config.read().unwrap().clone());
    ObsCaptionService::new(getter, None)
}

async fn start_with_mock(
    config: serde_json::Value,
    mock: MockObsClient,
) -> (
    Arc<ObsCaptionService>,
    std::sync::Arc<std::sync::Mutex<Vec<(String, serde_json::Value)>>>,
) {
    let service = make_service(config);
    service.start().await;
    let requests = mock.requests.clone();
    service.install_mock_client(mock).await;
    (service, requests)
}

fn translation_payload(text: &str) -> SubtitlePayloadEvent {
    SubtitlePayloadEvent {
        sequence: 1,
        source_lang: "ru".into(),
        source_text: "Привет".into(),
        display_order: vec!["en".into()],
        show_source: false,
        show_translations: true,
        max_translation_languages: 1,
        items: vec![SubtitleLineItem {
            kind: "translation".into(),
            lang: "en".into(),
            label: "EN".into(),
            text: text.into(),
            style_slot: Some("translation_1".into()),
            slot_id: Some("translation_1".into()),
            target_lang: Some("en".into()),
            provider: None,
            visible: true,
            success: true,
            error: None,
        }],
        visible_items: vec![SubtitleLineItem {
            kind: "translation".into(),
            lang: "en".into(),
            label: "EN".into(),
            text: text.into(),
            style_slot: Some("translation_1".into()),
            slot_id: Some("translation_1".into()),
            target_lang: Some("en".into()),
            provider: None,
            visible: true,
            success: true,
            error: None,
        }],
        lifecycle_state: LifecycleState::CompletedOnly,
        completed_block_visible: true,
        line1: text.into(),
        ..SubtitlePayloadEvent::default()
    }
}

fn request_count(
    requests: &std::sync::Arc<std::sync::Mutex<Vec<(String, serde_json::Value)>>>,
    request_type: &str,
) -> usize {
    requests
        .lock()
        .unwrap()
        .iter()
        .filter(|(kind, _)| kind == request_type)
        .count()
}

async fn wait_for_requests(
    requests: &std::sync::Arc<std::sync::Mutex<Vec<(String, serde_json::Value)>>>,
    request_type: &str,
    min_count: usize,
) {
    for _ in 0..50 {
        if request_count(requests, request_type) >= min_count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for {request_type} x{min_count}");
}

#[tokio::test]
async fn send_text_delivers_stream_caption_via_mock_transport() {
    let config = Arc::new(RwLock::new(obs_config("translation_1", false)));
    let getter: ConfigGetter = {
        let config = config.clone();
        Arc::new(move || config.read().unwrap().clone())
    };
    let service = ObsCaptionService::new(getter, None);
    service.start().await;

    let mock = MockObsClient::new();
    let requests = mock.requests.clone();
    service.install_mock_client(mock).await;

    service.publish_payload(translation_payload("Hello world"));
    wait_for_requests(&requests, "SendStreamCaption", 1).await;
}

#[tokio::test]
async fn send_text_dedupes_identical_stream_captions() {
    let config = Arc::new(RwLock::new(obs_config("translation_1", false)));
    let getter: ConfigGetter = {
        let config = config.clone();
        Arc::new(move || config.read().unwrap().clone())
    };
    let service = ObsCaptionService::new(getter, None);
    service.start().await;

    let mock = MockObsClient::new();
    let requests = mock.requests.clone();
    service.install_mock_client(mock).await;

    service.publish_payload(translation_payload("Hello world"));
    wait_for_requests(&requests, "SendStreamCaption", 1).await;

    let mut second = translation_payload("Hello world");
    second.sequence = 2;
    service.publish_payload(second);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let caption_sends = request_count(&requests, "SendStreamCaption");
    assert_eq!(caption_sends, 1, "duplicate caption text should be deduped");
}

#[tokio::test]
async fn send_text_dedupes_identical_debug_mirror_updates() {
    let config = Arc::new(RwLock::new(obs_config("first_visible_line", true)));
    let getter: ConfigGetter = {
        let config = config.clone();
        Arc::new(move || config.read().unwrap().clone())
    };
    let service = ObsCaptionService::new(getter, None);
    service.start().await;

    let mock = MockObsClient::new();
    let requests = mock.requests.clone();
    service.install_mock_client(mock).await;

    let payload = translation_payload("Mirror me");
    service.publish_payload(payload.clone());
    wait_for_requests(&requests, "SetInputSettings", 1).await;

    let mut second = payload;
    second.sequence = 2;
    service.publish_payload(second);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mirror_sends = request_count(&requests, "SetInputSettings");
    assert_eq!(
        mirror_sends, 1,
        "duplicate debug mirror text should be deduped"
    );
}

fn debug_mirror_texts(
    requests: &std::sync::Arc<std::sync::Mutex<Vec<(String, serde_json::Value)>>>,
) -> Vec<String> {
    requests
        .lock()
        .unwrap()
        .iter()
        .filter(|(kind, _)| kind == "SetInputSettings")
        .filter_map(|(_, data)| {
            data.get("inputSettings")?
                .get("text")?
                .as_str()
                .map(str::to_string)
        })
        .collect()
}

#[tokio::test]
async fn source_live_partial_debug_keeps_growing_text_when_stream_inactive() {
    let config = Arc::new(RwLock::new(obs_source_live_config(true)));
    let getter: ConfigGetter = {
        let config = config.clone();
        Arc::new(move || config.read().unwrap().clone())
    };
    let service = ObsCaptionService::new(getter, None);
    service.start().await;

    let mut mock = MockObsClient::new();
    mock.fail_caption_inactive = true;
    let requests = mock.requests.clone();
    service.install_mock_client(mock).await;

    service.publish_source("П", false);
    wait_for_requests(&requests, "SetInputSettings", 1).await;
    service.publish_source("При", false);
    wait_for_requests(&requests, "SetInputSettings", 2).await;
    service.publish_source("Привет", false);
    wait_for_requests(&requests, "SetInputSettings", 3).await;

    let mirror_texts = debug_mirror_texts(&requests);
    assert_eq!(mirror_texts, vec!["П", "При", "Привет"]);
    assert!(
        !mirror_texts.iter().any(|text| text.is_empty()),
        "debug mirror must not be cleared between partials"
    );

    let caption_attempts = request_count(&requests, "SendStreamCaption");
    assert_eq!(
        caption_attempts, 1,
        "only the first partial should attempt native captions before stream is marked inactive"
    );
}

#[tokio::test]
async fn source_final_only_routes_final_caption_to_send_stream_caption() {
    let (service, requests) = start_with_mock(
        obs_base_config("source_final_only", false),
        MockObsClient::new(),
    )
    .await;

    service.publish_source("  hello \n world ", true);
    wait_for_requests(&requests, "SendStreamCaption", 1).await;

    assert_eq!(
        stream_caption_texts(&requests),
        vec!["hello\nworld".to_string()]
    );
}

#[tokio::test]
async fn translation_mode_skips_repeat_for_same_sequence() {
    let (service, requests) =
        start_with_mock(obs_config("translation_1", false), MockObsClient::new()).await;

    let initial = payload_for_sequence(
        7,
        vec![
            source_line_item("Привет"),
            translation_line_item("Hello", "translation_1", "en", "EN"),
        ],
    );
    let late = payload_for_sequence(
        7,
        vec![
            source_line_item("Привет"),
            translation_line_item("Hello", "translation_1", "en", "EN"),
            translation_line_item("Hallo", "translation_2", "de", "DE"),
        ],
    );

    service.publish_payload(initial);
    wait_for_requests(&requests, "SendStreamCaption", 1).await;
    service.publish_payload(late);
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(
        stream_caption_texts(&requests),
        vec!["Hello".to_string()],
        "same sequence + text should dedup at payload signature"
    );
}

#[tokio::test]
async fn translation_mode_allows_same_text_for_new_sequence_after_clear() {
    let (service, requests) = start_with_mock(
        obs_config_with_timing(
            "translation_1",
            false,
            json!({
                "send_partials": true,
                "partial_throttle_ms": 140,
                "min_partial_delta_chars": 1,
                "final_replace_delay_ms": 0,
                "clear_after_ms": 50,
                "avoid_duplicate_text": true
            }),
        ),
        MockObsClient::new(),
    )
    .await;

    let first = payload_for_sequence(
        7,
        vec![
            source_line_item("Привет"),
            translation_line_item("Hello", "translation_1", "en", "EN"),
        ],
    );
    let second = payload_for_sequence(
        8,
        vec![
            source_line_item("Пока"),
            translation_line_item("Hello", "translation_1", "en", "EN"),
        ],
    );

    service.publish_payload(first);
    wait_for_requests(&requests, "SendStreamCaption", 1).await;
    wait_for_requests(&requests, "SendStreamCaption", 2).await;
    service.publish_payload(second);
    wait_for_requests(&requests, "SendStreamCaption", 3).await;

    assert_eq!(
        stream_caption_texts(&requests),
        vec!["Hello".to_string(), String::new(), "Hello".to_string()],
        "after clear, same text for a new sequence should be sent again"
    );
}

#[tokio::test]
async fn source_live_partial_skips_duplicate_and_small_growth_within_throttle_window() {
    let (service, requests) =
        start_with_mock(obs_base_config("source_live", false), MockObsClient::new()).await;

    service.publish_source("Hello", false);
    wait_for_requests(&requests, "SendStreamCaption", 1).await;
    service.publish_source("Hello", false);
    service.publish_source("Hello!", false);
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(
        stream_caption_texts(&requests),
        vec!["Hello".to_string()],
        "duplicate and small growth within throttle window should be suppressed"
    );
}

#[tokio::test]
async fn source_live_partial_allows_new_word_within_throttle_window() {
    let (service, requests) = start_with_mock(
        obs_config_with_timing(
            "source_live",
            false,
            json!({
                "send_partials": true,
                "partial_throttle_ms": 1000,
                "min_partial_delta_chars": 8,
                "final_replace_delay_ms": 0,
                "clear_after_ms": 0,
                "avoid_duplicate_text": false
            }),
        ),
        MockObsClient::new(),
    )
    .await;

    service.publish_source("Hello", false);
    wait_for_requests(&requests, "SendStreamCaption", 1).await;
    service.publish_source("Hello cruel", false);
    wait_for_requests(&requests, "SendStreamCaption", 2).await;

    assert_eq!(
        stream_caption_texts(&requests),
        vec!["Hello".to_string(), "Hello cruel".to_string()]
    );
}

#[tokio::test]
async fn debug_mirror_dedups_set_input_settings_on_source_final() {
    let (service, requests) = start_with_mock(
        obs_base_config("source_final_only", true),
        MockObsClient::new(),
    )
    .await;

    service.publish_source("Hello", true);
    wait_for_requests(&requests, "SendStreamCaption", 1).await;
    service.publish_source("Hello", true);
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(request_count(&requests, "SetInputSettings"), 1);
    assert_eq!(request_count(&requests, "SendStreamCaption"), 1);
    assert_eq!(debug_mirror_texts(&requests), vec!["Hello".to_string()]);
}

#[tokio::test]
async fn stream_inactive_schedules_debug_mirror_clear_after_501() {
    let (service, requests) = start_with_mock(
        obs_config_with_timing(
            "source_final_only",
            true,
            json!({
                "send_partials": true,
                "partial_throttle_ms": 140,
                "min_partial_delta_chars": 1,
                "final_replace_delay_ms": 0,
                "clear_after_ms": 50,
                "avoid_duplicate_text": false
            }),
        ),
        {
            let mut mock = MockObsClient::new();
            mock.fail_caption_inactive = true;
            mock
        },
    )
    .await;

    service.publish_source("Hello", true);
    wait_for_requests(&requests, "SetInputSettings", 1).await;
    wait_for_requests(&requests, "SetInputSettings", 2).await;

    assert_eq!(
        debug_mirror_texts(&requests),
        vec!["Hello".to_string(), String::new()],
        "debug mirror should clear after 501 when stream is inactive"
    );
}

#[tokio::test]
async fn schedule_final_send_cancels_stale_clear_from_previous_caption() {
    let (service, requests) = start_with_mock(
        obs_config_with_timing(
            "source_final_only",
            false,
            json!({
                "send_partials": true,
                "partial_throttle_ms": 140,
                "min_partial_delta_chars": 1,
                "final_replace_delay_ms": 0,
                "clear_after_ms": 120,
                "avoid_duplicate_text": false
            }),
        ),
        MockObsClient::new(),
    )
    .await;

    service.publish_source("Hello", true);
    wait_for_requests(&requests, "SendStreamCaption", 1).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    service.publish_source("World", true);
    wait_for_requests(&requests, "SendStreamCaption", 2).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(
        stream_caption_texts(&requests),
        vec!["Hello".to_string(), "World".to_string(), String::new()],
        "stale clear from the first caption must not fire after the second caption replaces it"
    );
}

#[tokio::test]
async fn deduped_payload_does_not_cancel_pending_clear() {
    let (service, requests) = start_with_mock(
        obs_config_with_timing(
            "translation_1",
            false,
            json!({
                "send_partials": true,
                "partial_throttle_ms": 140,
                "min_partial_delta_chars": 1,
                "final_replace_delay_ms": 0,
                "clear_after_ms": 80,
                "avoid_duplicate_text": true
            }),
        ),
        MockObsClient::new(),
    )
    .await;

    let payload = payload_for_sequence(
        7,
        vec![
            source_line_item("Привет"),
            translation_line_item("Hello", "translation_1", "en", "EN"),
        ],
    );

    service.publish_payload(payload.clone());
    wait_for_requests(&requests, "SendStreamCaption", 1).await;
    service.publish_payload(payload);
    wait_for_requests(&requests, "SendStreamCaption", 2).await;

    assert_eq!(
        stream_caption_texts(&requests),
        vec!["Hello".to_string(), String::new()],
        "deduped payload republish must not cancel the pending clear timer"
    );
}

#[tokio::test]
async fn partial_payload_does_not_cancel_pending_clear() {
    let (service, requests) = start_with_mock(
        obs_config_with_timing(
            "translation_1",
            false,
            json!({
                "send_partials": true,
                "partial_throttle_ms": 140,
                "min_partial_delta_chars": 1,
                "final_replace_delay_ms": 0,
                "clear_after_ms": 80,
                "avoid_duplicate_text": true
            }),
        ),
        MockObsClient::new(),
    )
    .await;

    let completed = payload_for_sequence(
        7,
        vec![
            source_line_item("Привет"),
            translation_line_item("Hello", "translation_1", "en", "EN"),
        ],
    );
    let mut partial = completed.clone();
    partial.completed_block_visible = false;
    partial.lifecycle_state = LifecycleState::PartialOnly;

    service.publish_payload(completed);
    wait_for_requests(&requests, "SendStreamCaption", 1).await;
    service.publish_payload(partial);
    wait_for_requests(&requests, "SendStreamCaption", 2).await;

    assert_eq!(
        stream_caption_texts(&requests),
        vec!["Hello".to_string(), String::new()],
        "partial overlay payload must not cancel the pending clear timer"
    );
}
