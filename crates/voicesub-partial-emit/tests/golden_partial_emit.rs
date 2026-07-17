use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;
use voicesub_partial_emit::{PartialEmitCoordinator, PartialEmitSettings};

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join("tests/golden/local_asr")
        .join(name)
}

fn load_fixture(name: &str) -> Value {
    let raw = fs::read_to_string(golden_path(name)).expect("read fixture");
    serde_json::from_str(&raw).expect("parse fixture")
}

#[derive(Debug, Deserialize)]
struct PartialEmitStep {
    segment_id: String,
    text: String,
    expect_emit: bool,
    #[serde(default)]
    min_new_words: Option<u32>,
}

#[test]
fn golden_partial_emit_word_growth() {
    let fixture = load_fixture("partial_emit_word_growth.json");
    let settings_raw = &fixture["settings"];
    let mut settings = PartialEmitSettings::from_fields(
        settings_raw["partial_emit_mode"].as_str().unwrap(),
        settings_raw["partial_min_new_words"].as_u64().unwrap() as u32,
        settings_raw["partial_min_delta_chars"].as_u64().unwrap() as u32,
        settings_raw["partial_coalescing_ms"].as_u64().unwrap_or(0) as u32,
    );
    let steps: Vec<PartialEmitStep> =
        serde_json::from_value(fixture["steps"].clone()).expect("steps");

    let mut coord = PartialEmitCoordinator::default();
    for step in steps {
        if let Some(min_words) = step.min_new_words {
            settings.partial_min_new_words = min_words;
        }
        let emitted = coord.should_emit(&settings, &step.segment_id, &step.text);
        assert_eq!(
            emitted, step.expect_emit,
            "segment={} text={:?}",
            step.segment_id, step.text
        );
    }
}
