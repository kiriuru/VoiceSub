use std::fs;
use std::io::Write;

use serde_json::json;
use voicesub_tts::{
    SpeechQueueItem, SubtitleSpeechPlanner, TtsModuleService, TtsSpeechSettings,
};

#[test]
fn service_plans_subtitle_lines_when_enabled() {
    let dir = std::env::temp_dir().join(format!("voicesub-tts-svc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let service = TtsModuleService::new(&dir);

    let payload = json!({
        "sequence": 1,
        "lifecycle_state": "completed_only",
        "visible_items": [
            {"kind": "source", "text": "Hello world"},
            {"kind": "translation", "text": "Привет мир", "slot_id": "translation_1"}
        ]
    });

    let planned = service.plan_subtitle_speech(&payload);
    assert_eq!(planned.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn service_skips_when_disabled() {
    let dir = std::env::temp_dir().join(format!("voicesub-tts-svc-off-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let service = TtsModuleService::new(&dir);
    service.set_enabled(false).expect("disable");

    let payload = json!({
        "sequence": 1,
        "lifecycle_state": "completed_only",
        "visible_items": [{"kind": "source", "text": "Hello"}]
    });

    assert!(service.plan_subtitle_speech(&payload).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn service_skips_when_config_unreadable() {
    let dir = std::env::temp_dir().join(format!(
        "voicesub-tts-bad-cfg-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    let config_dir = dir.join("modules").join("tts");
    fs::create_dir_all(&config_dir).expect("mkdir");
    let mut file = fs::File::create(config_dir.join("config.toml")).expect("create bad config");
    file.write_all(b"not-valid-toml [[[")
        .expect("write bad config");

    let service = TtsModuleService::new(&dir);
    let payload = json!({
        "sequence": 1,
        "lifecycle_state": "completed_only",
        "visible_items": [{"kind": "source", "text": "Hello"}]
    });
    assert!(service.plan_subtitle_speech(&payload).is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn enqueue_drop_releases_subtitle_dedupe_key() {
    let dir = std::env::temp_dir().join(format!(
        "voicesub-tts-drop-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    let service = TtsModuleService::new(&dir);
    service
        .update_speech_settings(TtsSpeechSettings {
            max_queue_items: 4,
            ..TtsSpeechSettings::default()
        })
        .expect("speech settings");

    let payload = json!({
        "sequence": 1,
        "lifecycle_state": "completed_only",
        "visible_items": [{"kind": "source", "text": "first line"}]
    });
    let planned = service.plan_subtitle_speech(&payload);
    assert_eq!(planned.len(), 1);
    service
        .enqueue_channel("speech", planned[0].clone())
        .expect("enqueue subtitle");

    let mut total_dropped = 0usize;
    for index in 0..12 {
        let result = service
            .enqueue_channel(
                "speech",
                SpeechQueueItem {
                    id: format!("filler-{index}"),
                    text: format!("filler {index}"),
                    source: "test".into(),
                    lang: "en".into(),
                    dedupe_key: None,
                },
            )
            .expect("enqueue filler");
        total_dropped += result.dropped_ids.len();
    }
    assert!(total_dropped > 0, "expected adaptive drop ids during saturation");

    let replay = service.plan_subtitle_speech(&payload);
    assert_eq!(replay.len(), 1, "dropped subtitle should be planned again");
    assert_eq!(replay[0].text, "first line");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn planner_reset_allows_repeat() {
    let mut planner = SubtitleSpeechPlanner::new();
    let settings = TtsSpeechSettings::default();
    let payload = json!({
        "sequence": 1,
        "lifecycle_state": "completed_only",
        "visible_items": [{"kind": "source", "text": "Repeat me"}]
    });
    assert_eq!(planner.plan(&payload, &settings).len(), 1);
    assert!(planner.plan(&payload, &settings).is_empty());
    planner.reset();
    assert_eq!(planner.plan(&payload, &settings).len(), 1);
}
