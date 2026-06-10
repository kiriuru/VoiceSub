use voicesub_browser::BrowserAsrDiagnostics;
use voicesub_runtime::http::partial_emit_settings_from_config;

#[test]
fn partial_emit_settings_read_from_config() {
    let config = serde_json::json!({
        "asr": {
            "realtime": {
                "partial_emit_mode": "char_delta",
                "partial_min_new_words": 3,
                "partial_coalescing_ms": 120
            }
        }
    });
    let settings = partial_emit_settings_from_config(&config);
    assert_eq!(settings.partial_emit_mode, "char_delta");
    assert_eq!(settings.partial_min_new_words, 3);
    assert_eq!(settings.partial_coalescing_ms, 120);
}

#[test]
fn browser_worker_default_is_disconnected() {
    let worker = BrowserAsrDiagnostics::default();
    assert!(!worker.worker_connected);
}
