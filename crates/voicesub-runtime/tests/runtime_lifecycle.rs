#![allow(clippy::await_holding_lock)]

mod common;

use std::time::Duration;

use common::{AuthedApi, EphemeralRuntime, integration_lock};
use voicesub_runtime::LOOPBACK_TOKEN_HEADER;

#[tokio::test]
async fn runtime_start_and_stop_serves_health() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let api = AuthedApi::new(&client, &runtime.service);
    let response = api
        .get(format!("http://{addr}/api/health"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("health request");
    assert!(response.status().is_success());

    handle.shutdown().await;
}

#[tokio::test]
async fn protected_api_rejects_missing_token() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{addr}/api/settings/load"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("settings load");
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);

    handle.shutdown().await;
}

#[tokio::test]
async fn protected_api_rejects_invalid_token() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{addr}/api/settings/load"))
        .header(LOOPBACK_TOKEN_HEADER, "not-the-session-token")
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("settings load");
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);

    handle.shutdown().await;
}

#[tokio::test]
async fn runtime_settings_load_after_start() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let api = AuthedApi::new(&client, &runtime.service);
    let response = api
        .get(format!("http://{addr}/api/settings/load"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("settings load");
    assert!(response.status().is_success());
    let body: serde_json::Value = response.json().await.expect("json");
    assert!(body.get("payload").is_some());

    handle.shutdown().await;
}

#[tokio::test]
async fn protected_health_rejects_missing_token() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{addr}/api/health"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("health request");
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);

    handle.shutdown().await;
}

#[tokio::test]
async fn public_live_endpoint_without_token() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{addr}/live"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("live request");
    assert!(response.status().is_success());
    let body: serde_json::Value = response.json().await.expect("json");
    assert_eq!(body["ok"], true);

    handle.shutdown().await;
}

#[tokio::test]
async fn trusted_dashboard_html_injects_loopback_token() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{addr}/"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("dashboard index");
    assert!(response.status().is_success());
    let html = response.text().await.expect("html body");
    assert!(
        html.contains("__VOICESUB_API_TOKEN__"),
        "dashboard HTML must inject loopback API token"
    );

    handle.shutdown().await;
}

#[tokio::test]
async fn trusted_worker_html_injects_loopback_token() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{addr}/google-asr"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("worker page");
    assert!(response.status().is_success());
    let html = response.text().await.expect("html body");
    assert!(
        html.contains("__VOICESUB_API_TOKEN__"),
        "worker HTML must inject loopback API token"
    );

    handle.shutdown().await;
}

#[tokio::test]
async fn trusted_tts_html_injects_loopback_token() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{addr}/tts"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("tts page");
    assert!(response.status().is_success());
    let html = response.text().await.expect("html body");
    assert!(
        html.contains("__VOICESUB_API_TOKEN__"),
        "tts HTML must inject loopback API token"
    );

    handle.shutdown().await;
}

fn loopback_token_from_trusted_html(html: &str) -> Option<String> {
    let needle = "window.__VOICESUB_API_TOKEN__=";
    let start = html.find(needle)? + needle.len();
    let rest = html[start..].trim_start();
    let end = rest.find(';')?;
    serde_json::from_str::<String>(rest[..end].trim()).ok()
}

#[tokio::test]
async fn protected_api_accepts_token_from_trusted_html_injection() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;

    let client = reqwest::Client::new();
    let html = client
        .get(format!("http://{addr}/"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("dashboard index")
        .text()
        .await
        .expect("html body");
    let token = loopback_token_from_trusted_html(&html).expect("injected loopback token");

    let response = client
        .get(format!("http://{addr}/api/settings/load"))
        .header(LOOPBACK_TOKEN_HEADER, token)
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("settings load");
    assert!(response.status().is_success());

    handle.shutdown().await;
}

#[tokio::test]
async fn protected_api_accepts_runtime_token_without_html_injection() {
    let _guard = integration_lock();
    let runtime = EphemeralRuntime::new();
    let handle = runtime.start().await;
    let addr = handle.bind_addr;
    let token = runtime.service.loopback_api_token();

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{addr}/api/settings/load"))
        .header(LOOPBACK_TOKEN_HEADER, token)
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .expect("settings load");
    assert!(response.status().is_success());

    handle.shutdown().await;
}
