use std::fs;

use serde_json::json;
use voicesub_config::{normalize_config_payload, ConfigStore};

#[test]
fn preserves_disabled_translation_line_toml_roundtrip() {
    let dir = std::env::temp_dir().join(format!("voicesub-cfg-dis-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");

    let mut store = ConfigStore::new(&path);
    store.load_or_create().unwrap();

    let payload = normalize_config_payload(json!({
        "translation": {
            "enabled": true,
            "provider": "google_translate_v2",
            "lines": [
                {
                    "slot_id": "translation_1",
                    "enabled": true,
                    "target_lang": "en",
                    "provider": "google_translate_v2",
                    "label": "EN"
                },
                {
                    "slot_id": "translation_4",
                    "enabled": false,
                    "target_lang": "ko",
                    "provider": "experimental_google_web",
                    "label": "KO"
                }
            ],
            "target_languages": ["en"]
        },
        "subtitle_output": {
            "display_order": ["source", "translation_1"]
        }
    }));

    store.apply_save_payload(&payload).unwrap();

    let mut reloaded = ConfigStore::new(&path);
    reloaded.load_or_create().unwrap();

    let lines = reloaded.payload()["translation"]["lines"]
        .as_array()
        .expect("lines array");
    let line4 = lines
        .iter()
        .find(|line| line["slot_id"] == "translation_4")
        .expect("translation_4 line");
    assert_eq!(line4["enabled"], false);

    let targets = reloaded.payload()["translation"]["target_languages"]
        .as_array()
        .expect("target languages");
    assert!(!targets.iter().any(|lang| lang == "ko"));

    let _ = fs::remove_dir_all(dir);
}
