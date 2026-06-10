//! Microsoft Edge WebView2 runtime detection (Tauri shell dependency).

pub const WEBVIEW2_APP_GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

pub const WEBVIEW2_DOWNLOAD_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

const EDGE_CLIENTS_SUBKEY: &str = r"Software\Microsoft\EdgeUpdate\Clients";
const EDGE_CLIENTS_SUBKEY_WOW64: &str = r"Software\WOW6432Node\Microsoft\EdgeUpdate\Clients";

/// Returns installed Evergreen WebView2 version string (registry `pv`), if present.
pub fn installed_webview2_version() -> Option<String> {
    installed_webview2_version_from_registry().or_else(installed_webview2_version_from_disk)
}

#[cfg(windows)]
fn installed_webview2_version_from_registry() -> Option<String> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let client_key = format!("{EDGE_CLIENTS_SUBKEY}\\{WEBVIEW2_APP_GUID}");
    let client_key_wow64 = format!("{EDGE_CLIENTS_SUBKEY_WOW64}\\{WEBVIEW2_APP_GUID}");

    for (hive, subkey) in [
        (HKEY_LOCAL_MACHINE, client_key_wow64.as_str()),
        (HKEY_LOCAL_MACHINE, client_key.as_str()),
        (HKEY_CURRENT_USER, client_key.as_str()),
    ] {
        if let Some(version) = read_registry_pv(RegKey::predef(hive), subkey) {
            return Some(version);
        }
    }
    None
}

#[cfg(windows)]
fn read_registry_pv(root: winreg::RegKey, subkey: &str) -> Option<String> {
    let key = root.open_subkey_with_flags(subkey, winreg::enums::KEY_READ).ok()?;
    let value = key.get_value::<String, _>("pv").ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

#[cfg(not(windows))]
fn installed_webview2_version_from_registry() -> Option<String> {
    None
}

fn installed_webview2_version_from_disk() -> Option<String> {
    let roots = [
        std::env::var_os("ProgramFiles(x86)"),
        std::env::var_os("ProgramFiles"),
        std::env::var_os("LOCALAPPDATA"),
    ];
    for root in roots.into_iter().flatten() {
        let base = std::path::PathBuf::from(root).join("Microsoft").join("EdgeWebView");
        if !base.is_dir() {
            continue;
        }
        let entries = std::fs::read_dir(&base).ok()?;
        for entry in entries.flatten() {
            let exe = entry.path().join("msedgewebview2.exe");
            if exe.is_file() {
                return entry
                    .file_name()
                    .to_str()
                    .map(|name| name.to_string())
                    .or(Some("unknown".to_string()));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_url_is_https_microsoft_link() {
        assert!(WEBVIEW2_DOWNLOAD_URL.starts_with("https://"));
        assert!(WEBVIEW2_DOWNLOAD_URL.contains("microsoft"));
    }

    #[test]
    fn version_probe_does_not_panic() {
        let _ = installed_webview2_version();
    }
}
