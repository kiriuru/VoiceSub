use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::warn;

use crate::service::TtsModuleService;
use crate::TtsConfig;

pub const TTS_WINDOW_LABEL: &str = "tts";

pub fn validate_twitch_oauth_url(url: &str) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("url is empty".into());
    }
    if !trimmed.starts_with("https://id.twitch.tv/") {
        return Err("only Twitch OAuth URLs are allowed".into());
    }
    Ok(())
}

pub fn build_tts_module_url(bind_addr: SocketAddr) -> String {
    if bind_addr.ip().is_loopback() {
        format!("http://localhost:{}/tts", bind_addr.port())
    } else {
        format!("http://{}:{}/tts", bind_addr.ip(), bind_addr.port())
    }
}

pub fn tts_webview_data_dir(config_path: &std::path::Path) -> PathBuf {
    config_path
        .parent()
        .map(|dir| dir.join("webview2"))
        .unwrap_or_else(|| PathBuf::from("user-data/modules/tts/webview2"))
}

pub fn speech_queue_item_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    format!("tts-{millis}")
}

pub fn bind_window_process(
    service: &TtsModuleService,
    pid: u32,
) -> Result<TtsConfig, String> {
    if pid == 0 {
        warn!(target: "voicesub.tts.ipc", "unable to resolve TTS window process id");
        return Err("unable to resolve TTS window process id".to_string());
    }
    service.bind_window_process(pid).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twitch_oauth_url_allowlist() {
        assert!(validate_twitch_oauth_url("https://id.twitch.tv/oauth2/authorize").is_ok());
        assert!(validate_twitch_oauth_url("http://evil.example/").is_err());
        assert!(validate_twitch_oauth_url("").is_err());
    }

    #[test]
    fn tts_module_url_prefers_localhost_for_loopback() {
        let addr: SocketAddr = "127.0.0.1:8765".parse().expect("addr");
        assert_eq!(build_tts_module_url(addr), "http://localhost:8765/tts");
    }
}
