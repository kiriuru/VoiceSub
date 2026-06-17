use serde_json::json;
use voicesub_obs::{
    ObsCaptionSettings, normalize_text, select_payload_text, should_throttle_partial_update,
};
use voicesub_subtitle::{LifecycleState, SubtitleLineItem, SubtitlePayloadEvent};

fn sample_payload() -> SubtitlePayloadEvent {
    let items = vec![
        SubtitleLineItem {
            kind: "source".into(),
            lang: "ru".into(),
            label: "RU".into(),
            text: "Привет".into(),
            style_slot: Some("source".into()),
            slot_id: None,
            target_lang: None,
            provider: None,
            visible: true,
            success: true,
            error: None,
        },
        SubtitleLineItem {
            kind: "translation".into(),
            lang: "en".into(),
            label: "EN".into(),
            text: "Hello".into(),
            style_slot: Some("translation_1".into()),
            slot_id: Some("translation_1".into()),
            target_lang: Some("en".into()),
            provider: None,
            visible: true,
            success: true,
            error: None,
        },
        SubtitleLineItem {
            kind: "translation".into(),
            lang: "de".into(),
            label: "DE".into(),
            text: "Hallo".into(),
            style_slot: Some("translation_2".into()),
            slot_id: Some("translation_2".into()),
            target_lang: Some("de".into()),
            provider: None,
            visible: true,
            success: true,
            error: None,
        },
    ];
    SubtitlePayloadEvent {
        sequence: 1,
        source_lang: "ru".into(),
        source_text: "Привет".into(),
        display_order: vec!["source".into(), "en".into(), "de".into()],
        show_source: true,
        show_translations: true,
        max_translation_languages: 2,
        items: items.clone(),
        visible_items: items,
        lifecycle_state: LifecycleState::CompletedOnly,
        completed_block_visible: true,
        line1: "Привет".into(),
        line2: "Hello\nHallo".into(),
        ..SubtitlePayloadEvent::default()
    }
}

#[test]
fn settings_default_timing_matches_sst() {
    let settings = ObsCaptionSettings::from_config(&json!({
        "obs_closed_captions": { "enabled": true, "output_mode": "source_live" }
    }));
    assert_eq!(settings.partial_throttle_ms, 140);
    assert_eq!(settings.min_partial_delta_chars, 1);
}

#[test]
fn translation_mode_selects_requested_visible_translation() {
    let payload = sample_payload();
    assert_eq!(select_payload_text(&payload, "translation_2"), "Hallo");
}

#[test]
fn source_final_normalizes_whitespace() {
    assert_eq!(normalize_text("  hello \n world "), "hello\nworld");
}

#[test]
fn partial_throttle_skips_small_growth() {
    assert!(should_throttle_partial_update(
        "Hello",
        "Hello!",
        Some(50),
        1000,
        3
    ));
}

#[test]
fn partial_throttle_allows_new_word() {
    assert!(!should_throttle_partial_update(
        "Hello",
        "Hello cruel",
        Some(50),
        1000,
        8
    ));
}
