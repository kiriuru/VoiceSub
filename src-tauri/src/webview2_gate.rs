//! Block Tauri startup when WebView2 runtime is missing and tell the user why.

use voicesub_browser::{
    installed_webview2_version, system_supported_ui_language, webview2_missing_dialog_copy,
    WEBVIEW2_DOWNLOAD_URL,
};

/// Returns `true` when the app may continue starting.
pub fn ensure_runtime_available() -> bool {
    if installed_webview2_version().is_some() {
        return true;
    }
    tracing::error!("WebView2 runtime not found; aborting startup");
    let locale = system_supported_ui_language();
    let copy = webview2_missing_dialog_copy(locale);
    show_missing_runtime_dialog(copy.title, copy.body);
    false
}

fn show_missing_runtime_dialog(title: &str, body: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK, MESSAGEBOX_STYLE};

    fn wide(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().chain(Some(0)).collect()
    }

    let title = wide(title);
    let body = wide(body);
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(body.as_ptr()),
            PCWSTR(title.as_ptr()),
            MESSAGEBOX_STYLE(MB_OK.0 | MB_ICONERROR.0),
        );
    }
    let _ = open::that(WEBVIEW2_DOWNLOAD_URL);
}
