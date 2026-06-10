#![allow(clippy::await_holding_lock)]

mod common;

use std::time::Duration;

use common::{integration_lock, EphemeralRuntime};

#[tokio::test]
async fn runtime_start_and_stop_serves_health() {
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
    assert!(response.status().is_success());

    handle.shutdown().await;
}

#[tokio::test]
async fn runtime_settings_load_after_start() {
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
    assert!(response.status().is_success());
    let body: serde_json::Value = response.json().await.expect("json");
    assert!(body.get("payload").is_some());

    handle.shutdown().await;
}
