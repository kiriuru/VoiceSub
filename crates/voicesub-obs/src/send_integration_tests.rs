use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde_json::json;
use voicesub_subtitle::{ConfigGetter, LifecycleState, SubtitleLineItem, SubtitlePayloadEvent};

use crate::client::MockObsClient;
use crate::ObsCaptionService;

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
    assert_eq!(mirror_sends, 1, "duplicate debug mirror text should be deduped");
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
