//! Tauri shell — open/focus the Local ASR module webview (`/local-asr`).

use std::path::PathBuf;

use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tracing::info;
use voicesub_asr_local::{build_local_asr_module_url, LOCAL_ASR_WINDOW_LABEL};

pub struct LocalAsrState {
    pub bind_addr: std::net::SocketAddr,
}

fn local_asr_webview_data_dir(config_path: &std::path::Path) -> PathBuf {
    config_path
        .parent()
        .map(|p| p.join("webview2-local-asr"))
        .unwrap_or_else(|| PathBuf::from("webview2-local-asr"))
}

#[tauri::command]
pub async fn local_asr_open_window(
    app: AppHandle,
    state: State<'_, LocalAsrState>,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(LOCAL_ASR_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    let url = build_local_asr_module_url(state.bind_addr);
    info!(
        target: "voicesub.asr_local.ipc",
        url = %url,
        "creating local asr module window"
    );
    let parsed = url.parse::<url::Url>().map_err(|e| e.to_string())?;
    let data_dir = local_asr_webview_data_dir(std::path::Path::new("user-data/modules/local-asr/config.toml"));
    let _ = std::fs::create_dir_all(&data_dir);

    WebviewWindowBuilder::new(&app, LOCAL_ASR_WINDOW_LABEL, WebviewUrl::External(parsed))
        .title("VoiceSub Local ASR")
        .inner_size(820.0, 720.0)
        .min_inner_size(640.0, 520.0)
        .data_directory(data_dir)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Close the module window when the desktop shell shuts down (same lifecycle as TTS).
pub fn close_local_asr_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LOCAL_ASR_WINDOW_LABEL) else {
        return;
    };
    info!(
        target: "voicesub.asr_local.ipc",
        "closing local asr module window on app shutdown"
    );
    let _ = window.destroy();
}
