//! Apply WebView2 memory / suspend policy to Tauri windows (Windows only).

use std::sync::Mutex;

use tauri::{AppHandle, Manager, WebviewWindow};
use tracing::debug;
use voicesub_browser::{
    apply_from_controller, resolve_power_action, WebviewActivity, WebviewPowerAction, WebviewRole,
};
use voicesub_tts::TTS_WINDOW_LABEL;

#[derive(Debug, Default, Clone, Copy)]
struct WindowFlags {
    focused: bool,
    visible: bool,
}

#[derive(Debug, Default)]
pub struct WebviewMemoryManager {
    main: WindowFlags,
    tts: WindowFlags,
    runtime_active: bool,
    tts_enabled: bool,
    engines_busy: bool,
}

impl WebviewMemoryManager {
    pub fn set_main_focused(&mut self, focused: bool) {
        self.main.focused = focused;
    }

    pub fn set_tts_focused(&mut self, focused: bool) {
        self.tts.focused = focused;
    }

    pub fn set_tts_visible(&mut self, visible: bool) {
        self.tts.visible = visible;
    }

    pub fn set_tts_activity(
        &mut self,
        runtime_active: bool,
        tts_enabled: bool,
        engines_busy: bool,
    ) {
        self.runtime_active = runtime_active;
        self.tts_enabled = tts_enabled;
        self.engines_busy = engines_busy;
    }

    fn activity(&self, role: WebviewRole) -> WebviewActivity {
        let flags = match role {
            WebviewRole::MainShell => self.main,
            WebviewRole::TtsModule => self.tts,
        };
        WebviewActivity {
            focused: flags.focused,
            visible: flags.visible,
            runtime_active: self.runtime_active,
            tts_enabled: self.tts_enabled,
            engines_busy: self.engines_busy,
        }
    }
}

pub type SharedWebviewMemoryManager = Mutex<WebviewMemoryManager>;

fn apply_to_window(window: &WebviewWindow, _role: WebviewRole, action: WebviewPowerAction) {
    let label = window.label().to_string();
    let _ = window.with_webview(move |platform| {
        #[cfg(windows)]
        {
            apply_from_controller(&platform.controller(), action, &label);
        }
        #[cfg(not(windows))]
        {
            let _ = (platform, role, action, label);
        }
    });
}

pub fn refresh_webview_power(app: &AppHandle, manager: &WebviewMemoryManager) {
    if let Some(main) = app.get_webview_window("main") {
        let action = resolve_power_action(WebviewRole::MainShell, manager.activity(WebviewRole::MainShell));
        debug!(window = "main", ?action, "apply webview power");
        apply_to_window(&main, WebviewRole::MainShell, action);
    }

    if let Some(tts) = app.get_webview_window(TTS_WINDOW_LABEL) {
        let action = resolve_power_action(WebviewRole::TtsModule, manager.activity(WebviewRole::TtsModule));
        debug!(window = "tts", ?action, "apply webview power");
        apply_to_window(&tts, WebviewRole::TtsModule, action);
    }
}

pub fn refresh_from_state(app: &AppHandle, state: &SharedWebviewMemoryManager) {
    let Ok(guard) = state.lock() else {
        return;
    };
    refresh_webview_power(app, &guard);
}

pub fn sync_tts_window_visibility(app: &AppHandle, state: &SharedWebviewMemoryManager) {
    let Some(tts) = app.get_webview_window(TTS_WINDOW_LABEL) else {
        return;
    };
    let visible = tts.is_visible().unwrap_or(true);
    if let Ok(mut guard) = state.lock() {
        guard.set_tts_visible(visible);
    }
    refresh_from_state(app, state);
}

#[cfg(test)]
mod tests {
    use super::WebviewMemoryManager;
    use voicesub_browser::{resolve_power_action, WebviewPowerAction, WebviewRole};

    #[test]
    fn manager_activity_feeds_policy() {
        let mut manager = WebviewMemoryManager::default();
        manager.set_tts_activity(true, true, false);
        manager.tts.visible = false;
        manager.tts.focused = false;
        let action = resolve_power_action(WebviewRole::TtsModule, manager.activity(WebviewRole::TtsModule));
        assert_eq!(action, WebviewPowerAction::LowMemory);
    }
}
