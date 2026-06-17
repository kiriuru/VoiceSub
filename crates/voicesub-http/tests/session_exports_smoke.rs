#![allow(clippy::await_holding_lock)]

mod common;

use std::time::Duration;

use common::{EphemeralRuntime, integration_lock};

#[tokio::test]
async fn client_event_returns_json_logged_true() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let response = runtime
        .authed(&client)
        .post(format!("http://{addr}/api/logs/client-event"))
        .json(&serde_json::json!({
            "channel": "dashboard",
            "message": "smoke test event",
        }))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("client event");
    assert!(response.status().is_success());
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["logged"], true);

    handle.shutdown().await;
}

#[tokio::test]
async fn list_exports_returns_array() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let response = runtime
        .authed(&client)
        .get(format!("http://{addr}/api/exports"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("list exports");
    assert!(response.status().is_success());
    let body: serde_json::Value = response.json().await.expect("json");
    assert!(body["exports"].is_array());
    assert!(body["files"].is_array());

    handle.shutdown().await;
}

#[tokio::test]
async fn diagnostics_export_returns_zip() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let response = runtime
        .authed(&client)
        .get(format!("http://{addr}/api/exports/diagnostics"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("diagnostics export");
    assert!(response.status().is_success());
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("zip") || content_type.contains("octet-stream"),
        "unexpected content type: {content_type}"
    );

    handle.shutdown().await;
}

#[tokio::test]
async fn settings_roundtrip_preserves_payload_shape() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let load = runtime
        .authed(&client)
        .get(format!("http://{addr}/api/settings/load"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("settings load");
    assert!(load.status().is_success());
    let loaded: serde_json::Value = load.json().await.expect("json");
    assert_eq!(loaded["ok"], true);
    assert!(loaded.get("payload").is_some());
    assert!(loaded.get("subtitle_style_presets").is_some());
    assert!(loaded.get("font_catalog").is_some());

    let save = runtime
        .authed(&client)
        .post(format!("http://{addr}/api/settings/save"))
        .json(&serde_json::json!({
            "payload": loaded["payload"]
        }))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .expect("settings save");
    assert!(save.status().is_success());
    let saved: serde_json::Value = save.json().await.expect("json");
    assert_eq!(saved["ok"], true);
    assert_eq!(saved["live_applied"], true);

    handle.shutdown().await;
}

#[tokio::test]
async fn settings_save_accepts_libretranslate_provider() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let load = runtime
        .authed(&client)
        .get(format!("http://{addr}/api/settings/load"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("settings load");
    let loaded: serde_json::Value = load.json().await.expect("json");
    let mut payload = loaded["payload"].clone();
    payload["translation"]["enabled"] = serde_json::json!(true);
    payload["translation"]["provider"] = serde_json::json!("libretranslate");
    payload["translation"]["lines"] = serde_json::json!([{
        "slot_id": "translation_1",
        "enabled": true,
        "target_lang": "en",
        "provider": "libretranslate",
        "label": "EN"
    }]);
    payload["translation"]["provider_settings"]["libretranslate"] = serde_json::json!({
        "api_key": "",
        "api_url": "https://libretranslate.com/translate"
    });

    let save = runtime
        .authed(&client)
        .post(format!("http://{addr}/api/settings/save"))
        .json(&serde_json::json!({ "payload": payload }))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .expect("settings save");
    assert!(
        save.status().is_success(),
        "settings save failed: {}",
        save.status()
    );
    let saved: serde_json::Value = save.json().await.expect("json");
    assert_eq!(saved["ok"], true);
    assert_eq!(
        saved["payload"]["translation"]["lines"][0]["provider"],
        "libretranslate"
    );

    handle.shutdown().await;
}

#[tokio::test]
async fn settings_save_accepts_public_libretranslate_mirror_provider() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let load = runtime
        .authed(&client)
        .get(format!("http://{addr}/api/settings/load"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("settings load");
    let loaded: serde_json::Value = load.json().await.expect("json");
    let mut payload = loaded["payload"].clone();
    payload["translation"]["enabled"] = serde_json::json!(true);
    payload["translation"]["provider"] = serde_json::json!("public_libretranslate_mirror");
    payload["translation"]["lines"] = serde_json::json!([
        {
            "slot_id": "translation_1",
            "enabled": true,
            "target_lang": "ja",
            "provider": "public_libretranslate_mirror",
            "label": "JA"
        },
        {
            "slot_id": "translation_2",
            "enabled": true,
            "target_lang": "en",
            "provider": "google_web",
            "label": "EN"
        }
    ]);
    payload["translation"]["provider_settings"]["public_libretranslate_mirror"] = serde_json::json!({
        "api_url": "https://translate.fedilab.app/translate"
    });
    payload["obs_closed_captions"]["enabled"] = serde_json::json!(true);

    let save = runtime
        .authed(&client)
        .post(format!("http://{addr}/api/settings/save"))
        .json(&serde_json::json!({ "payload": payload }))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .expect("settings save");
    assert!(
        save.status().is_success(),
        "settings save failed: {} body={:?}",
        save.status(),
        save.text().await.ok()
    );
    let saved: serde_json::Value = save.json().await.expect("json");
    assert_eq!(saved["ok"], true);
    assert_eq!(
        saved["payload"]["translation"]["lines"][0]["provider"],
        "public_libretranslate_mirror"
    );
    assert_eq!(saved["live_applied"], true);

    handle.shutdown().await;
}
