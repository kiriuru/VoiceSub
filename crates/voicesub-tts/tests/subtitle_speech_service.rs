use serde_json::json;
use voicesub_tts::{SubtitleSpeechPlanner, TtsModuleService, TtsSpeechSettings};

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
