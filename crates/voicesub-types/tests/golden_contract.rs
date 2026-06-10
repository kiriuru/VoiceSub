use std::fs;
use std::path::Path;

use voicesub_types::{AsrWorkerHello, ExternalAsrUpdate, WsMessage};

fn golden_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace")
        .join("tests/golden")
}

#[test]
fn golden_ws_events_hello_matches_runtime() {
    let path = golden_dir().join("ws_events_hello.json");
    assert!(
        path.is_file(),
        "missing fixture {path:?}; run: cargo run -p xtask -- export-golden"
    );
    let raw = fs::read_to_string(path).expect("read fixture");
    let fixture: serde_json::Value = serde_json::from_str(&raw).expect("json");
    let expected: WsMessage =
        serde_json::from_value(fixture["expected"].clone()).expect("expected shape");
    assert_eq!(expected, WsMessage::hello_events());
}

#[test]
fn golden_asr_worker_hello_matches_sst() {
    let path = golden_dir().join("ws_asr_worker_hello.json");
    assert!(
        path.is_file(),
        "missing fixture {path:?}; run: cargo run -p xtask -- export-golden"
    );
    let raw = fs::read_to_string(path).expect("read");
    let fixture: serde_json::Value = serde_json::from_str(&raw).expect("json");
    let transport_id = fixture["input"]["transport_id"].as_u64().unwrap_or(1);
    let expected: AsrWorkerHello =
        serde_json::from_value(fixture["expected"].clone()).expect("expected");
    assert_eq!(expected, AsrWorkerHello::new(transport_id));
}

#[test]
fn golden_external_asr_update_semantics() {
    let path = golden_dir().join("external_asr_update_final.json");
    assert!(
        path.is_file(),
        "missing fixture {path:?}; run: cargo run -p xtask -- export-golden"
    );
    let raw = fs::read_to_string(path).expect("read");
    let fixture: serde_json::Value = serde_json::from_str(&raw).expect("json");
    let input: ExternalAsrUpdate = serde_json::from_value(fixture["input"].clone()).expect("input");
    let expected = &fixture["expected"];
    assert_eq!(
        input.transcript_text(),
        expected["transcript_text"].as_str().unwrap()
    );
    assert_eq!(input.is_final, expected["is_final"].as_bool().unwrap());
}
