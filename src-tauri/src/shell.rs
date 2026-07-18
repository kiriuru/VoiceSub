use tracing::info;
use url::Host;

/// Allow local overlay / worker preview pages opened from the dashboard shell.
pub fn validate_local_http_url(url: &str) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("url is empty".into());
    }
    let parsed = url::Url::parse(trimmed).map_err(|err| err.to_string())?;
    if parsed.scheme() != "http" {
        return Err("only http URLs are allowed".into());
    }
    let host = parsed.host().ok_or_else(|| "missing host".to_string())?;
    if !is_loopback_host(&host) {
        return Err(format!("host is not loopback: {host}"));
    }
    Ok(())
}

fn is_loopback_host(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(name) => name.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(ip) => ip.is_loopback(),
        Host::Ipv6(ip) => ip.is_loopback(),
    }
}

/// HTTPS hosts the desktop shell may open in the system browser.
/// Keep this explicit — the dashboard must not open arbitrary remote URLs.
const ALLOWED_EXTERNAL_HTTPS_HOSTS: &[&str] = &[
    // Updates / OAuth / GPU drivers
    "github.com",
    "www.github.com",
    "id.twitch.tv",
    "developer.nvidia.com",
    "www.nvidia.com",
    "nvidia.com",
    // Translation provider consoles / API key pages
    "console.cloud.google.com",
    "script.google.com",
    "portal.azure.com",
    "www.deepl.com",
    "deepl.com",
    "libretranslate.com",
    "www.libretranslate.com",
    "platform.openai.com",
    "openrouter.ai",
    "www.openrouter.ai",
    "fanyi-api.baidu.com",
    "ai.youdao.com",
    "console.cloud.tencent.com",
    "fanyi.caiyunapp.com",
];

/// Allow HTTPS release / OAuth / provider-setup pages opened from the dashboard shell.
pub fn validate_external_https_url(url: &str) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("url is empty".into());
    }
    let parsed = url::Url::parse(trimmed).map_err(|err| err.to_string())?;
    if parsed.scheme() != "https" {
        return Err("only https URLs are allowed".into());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "missing host".to_string())?
        .to_ascii_lowercase();
    if !ALLOWED_EXTERNAL_HTTPS_HOSTS
        .iter()
        .any(|allowed| host == *allowed)
    {
        return Err(format!("host is not allowed: {host}"));
    }
    Ok(())
}

#[tauri::command]
pub fn open_external_https_url(url: String) -> Result<(), String> {
    validate_external_https_url(&url)?;
    let trimmed = url.trim();
    info!(target: "voicesub.shell", url = %trimmed, "opening external https url");
    open::that(trimmed).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn open_local_http_url(url: String) -> Result<(), String> {
    validate_local_http_url(&url)?;
    let trimmed = url.trim();
    info!(target: "voicesub.shell", url = %trimmed, "opening local http url");
    open::that(trimmed).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_github_release_urls() {
        assert!(
            validate_external_https_url("https://github.com/kiriuru/VoiceSub/releases/tag/v0.5.2")
                .is_ok()
        );
    }

    #[test]
    fn allows_cuda_toolkit_download_urls() {
        assert!(validate_external_https_url(
            "https://developer.nvidia.com/cuda-13-0-0-download-archive?target_os=Windows&target_arch=x86_64&target_type=exe_local"
        )
        .is_ok());
        assert!(validate_external_https_url("https://www.nvidia.com/Download/index.aspx").is_ok());
    }

    #[test]
    fn allows_translation_provider_setup_urls() {
        assert!(validate_external_https_url("https://platform.openai.com/api-keys").is_ok());
        assert!(
            validate_external_https_url("https://console.cloud.google.com/apis/credentials")
                .is_ok()
        );
        assert!(validate_external_https_url(
            "https://portal.azure.com/#view/Microsoft_Azure_ProjectOxford/CognitiveServicesHub/~/TextTranslation"
        )
        .is_ok());
        assert!(validate_external_https_url("https://fanyi-api.baidu.com/").is_ok());
        assert!(validate_external_https_url("https://ai.youdao.com/").is_ok());
        assert!(validate_external_https_url("https://console.cloud.tencent.com/tmt").is_ok());
        assert!(validate_external_https_url("https://fanyi.caiyunapp.com/").is_ok());
        assert!(validate_external_https_url("https://openrouter.ai/keys").is_ok());
        assert!(validate_external_https_url("https://www.deepl.com/pro-api").is_ok());
    }

    #[test]
    fn rejects_non_https_and_unknown_hosts() {
        assert!(validate_external_https_url("http://github.com/foo").is_err());
        assert!(validate_external_https_url("https://evil.example/").is_err());
    }

    #[test]
    fn allows_loopback_overlay_urls() {
        assert!(validate_local_http_url("http://127.0.0.1:8765/overlay").is_ok());
        assert!(validate_local_http_url("http://localhost:8765/overlay?foo=1").is_ok());
        assert!(validate_local_http_url("http://[::1]:8765/overlay").is_ok());
    }

    #[test]
    fn rejects_non_loopback_local_http_urls() {
        assert!(validate_local_http_url("https://127.0.0.1:8765/overlay").is_err());
        assert!(validate_local_http_url("http://192.168.1.10:8765/overlay").is_err());
        assert!(validate_local_http_url("http://evil.example/overlay").is_err());
    }
}
