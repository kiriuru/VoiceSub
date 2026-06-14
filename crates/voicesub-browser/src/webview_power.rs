//! Pure policy for WebView2 memory / suspend actions (testable without COM).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebviewRole {
    MainShell,
    TtsModule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WebviewActivity {
    pub focused: bool,
    pub visible: bool,
    pub runtime_active: bool,
    pub tts_enabled: bool,
    pub engines_busy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebviewPowerAction {
    /// Full activity — resume if suspended and restore normal memory target.
    Normal,
    /// Scripts still run (WS, timers) but memory target is low.
    LowMemory,
    /// Deep idle — TrySuspend when the host supports it.
    Suspend,
}

/// Resolve the WebView2 power action for a shell window.
pub fn resolve_power_action(role: WebviewRole, activity: WebviewActivity) -> WebviewPowerAction {
    match role {
        WebviewRole::MainShell => resolve_main_shell(activity),
        WebviewRole::TtsModule => resolve_tts_module(activity),
    }
}

fn resolve_main_shell(activity: WebviewActivity) -> WebviewPowerAction {
    if activity.focused {
        return WebviewPowerAction::Normal;
    }
    WebviewPowerAction::LowMemory
}

fn resolve_tts_module(activity: WebviewActivity) -> WebviewPowerAction {
    if activity.engines_busy {
        return WebviewPowerAction::Normal;
    }
    // Module enabled: keep renderer alive for keepalive/WS even without subtitle runtime.
    if activity.tts_enabled {
        return WebviewPowerAction::Normal;
    }
    WebviewPowerAction::Suspend
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity(
        focused: bool,
        visible: bool,
        runtime_active: bool,
        tts_enabled: bool,
        engines_busy: bool,
    ) -> WebviewActivity {
        WebviewActivity {
            focused,
            visible,
            runtime_active,
            tts_enabled,
            engines_busy,
        }
    }

    #[test]
    fn main_shell_focused_is_normal() {
        assert_eq!(
            resolve_power_action(
                WebviewRole::MainShell,
                activity(true, true, false, false, false)
            ),
            WebviewPowerAction::Normal
        );
    }

    #[test]
    fn main_shell_unfocused_is_low_memory() {
        assert_eq!(
            resolve_power_action(
                WebviewRole::MainShell,
                activity(false, true, true, true, false)
            ),
            WebviewPowerAction::LowMemory
        );
    }

    #[test]
    fn tts_busy_is_normal() {
        assert_eq!(
            resolve_power_action(
                WebviewRole::TtsModule,
                activity(false, false, true, true, true)
            ),
            WebviewPowerAction::Normal
        );
    }

    #[test]
    fn tts_listening_hidden_stays_normal() {
        assert_eq!(
            resolve_power_action(
                WebviewRole::TtsModule,
                activity(false, false, true, true, false)
            ),
            WebviewPowerAction::Normal
        );
    }

    #[test]
    fn tts_enabled_without_runtime_stays_normal() {
        assert_eq!(
            resolve_power_action(
                WebviewRole::TtsModule,
                activity(false, false, false, true, false)
            ),
            WebviewPowerAction::Normal
        );
    }

    #[test]
    fn tts_fully_idle_is_suspend() {
        assert_eq!(
            resolve_power_action(
                WebviewRole::TtsModule,
                activity(false, false, false, false, false)
            ),
            WebviewPowerAction::Suspend
        );
    }
}
