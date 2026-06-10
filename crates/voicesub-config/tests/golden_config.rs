use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use voicesub_config::{import_sst_json_value, migrate_sst_payload};

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join("tests/golden")
        .join(name)
}

fn load_fixture(name: &str) -> Value {
    let path = golden_path(name);
    assert!(
        path.is_file(),
        "missing golden fixture {name}; run: cargo run -p xtask -- export-golden"
    );
    let raw = fs::read_to_string(path).expect("read fixture");
    serde_json::from_str(&raw).expect("parse fixture")
}

#[test]
fn golden_config_import_local_asr() {
    let fixture = load_fixture("config_import_local_asr.json");
    let expected = &fixture["expected"];
    let actual = import_sst_json_value(fixture["input"].clone());
    assert_eq!(actual["asr"]["mode"], "browser_google");
    assert!(actual.get("remote").is_none());
    assert_eq!(actual["config_version"], expected["config_version"]);
}

#[test]
fn golden_config_migrate_legacy_targets() {
    let fixture = load_fixture("config_migrate_legacy_targets.json");
    let actual = migrate_sst_payload(fixture["input"].clone());
    let expected = &fixture["expected"];
    assert_eq!(actual["config_version"], expected["config_version"]);
    assert_eq!(
        actual["translation"]["target_languages"],
        expected["translation"]["target_languages"]
    );
}

#[test]
fn golden_config_migrate_unversioned_schema() {
    use voicesub_config::import_sst_json_value;

    let fixture = load_fixture("config_migrate_unversioned_schema.json");
    let expected = &fixture["expected"];
    let actual = import_sst_json_value(fixture["input"].clone());

    assert!(actual.get("source_text_replacement").is_some());
    assert_eq!(
        actual["source_text_replacement"]["enabled"].as_bool(),
        expected["source_text_replacement_enabled"].as_bool()
    );
    assert!(actual.get("ui").is_some());
    assert!(actual.get("asr").is_some());
    assert!(actual.get("translation").is_some());
    assert!(actual.get("remote").is_none());
    assert_eq!(
        actual["translation"]["target_languages"],
        serde_json::json!(expected["translation_target_languages"])
    );
    assert_eq!(
        actual["translation"]["lines"].as_array().map(|a| a.len()),
        Some(expected["translation_lines_count"].as_u64().unwrap() as usize)
    );
    let slots: Vec<_> = actual["translation"]["lines"]
        .as_array()
        .expect("lines")
        .iter()
        .filter_map(|line| line.get("slot_id").and_then(|v| v.as_str()))
        .collect();
    let expected_slots: Vec<_> = expected["translation_line_slots"]
        .as_array()
        .expect("slots")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(slots, expected_slots);
    assert!(actual["subtitle_style"]["custom_presets"].get("stream").is_some());
}

#[test]
fn golden_config_import_strips_parakeet_fields() {
    use voicesub_config::import_sst_json_value;

    let fixture = load_fixture("config_migrate_parakeet_provider.json");
    let expected = &fixture["expected"];
    let actual = import_sst_json_value(fixture["input"].clone());

    assert_eq!(actual["asr"]["mode"], expected["asr_mode"]);
    assert!(
        actual["asr"].get("provider_preference").is_none(),
        "parakeet provider_preference must be stripped on import"
    );
}
