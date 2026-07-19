//! WebView2 memory target level and TrySuspend/Resume (Windows only).

#[cfg(windows)]
mod imp {
    use tracing::{debug, warn};
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL, ICoreWebView2, ICoreWebView2_3, ICoreWebView2_19,
        ICoreWebView2Controller,
    };
    use webview2_com::TrySuspendCompletedHandler;
    use windows::core::BOOL;
    use windows::core::Interface;

    use crate::webview_power::WebviewPowerAction;

    pub fn apply_from_controller(
        controller: &ICoreWebView2Controller,
        action: WebviewPowerAction,
        label: &str,
    ) {
        let webview = match unsafe { controller.CoreWebView2() } {
            Ok(webview) => webview,
            Err(err) => {
                warn!(label, error = %err, "webview CoreWebView2 unavailable");
                return;
            }
        };
        if let Err(err) = apply_to_webview(&webview, action, label) {
            if action == WebviewPowerAction::Suspend {
                debug!(
                    label,
                    ?action,
                    error = %err,
                    "webview TrySuspend skipped (webview not ready)"
                );
            } else {
                warn!(label, ?action, error = %err, "webview power action failed");
            }
        }
    }

    fn apply_to_webview(
        webview: &ICoreWebView2,
        action: WebviewPowerAction,
        label: &str,
    ) -> windows::core::Result<()> {
        match action {
            WebviewPowerAction::Normal => {
                resume_if_suspended(webview, label)?;
                set_memory_level(webview, false)?;
                debug!(label, "webview power normal");
            }
            WebviewPowerAction::LowMemory => {
                resume_if_suspended(webview, label)?;
                set_memory_level(webview, true)?;
                debug!(label, "webview power low memory");
            }
            WebviewPowerAction::Suspend => try_suspend(webview, label)?,
        }
        Ok(())
    }

    fn resume_if_suspended(webview: &ICoreWebView2, label: &str) -> windows::core::Result<()> {
        let Ok(webview3) = webview.cast::<ICoreWebView2_3>() else {
            return Ok(());
        };
        let mut suspended = BOOL(0);
        unsafe { webview3.IsSuspended(&raw mut suspended)? };
        if suspended.as_bool() {
            unsafe { webview3.Resume()? };
            debug!(label, "webview resumed");
        }
        Ok(())
    }

    fn set_memory_level(webview: &ICoreWebView2, low: bool) -> windows::core::Result<()> {
        let Ok(webview19) = webview.cast::<ICoreWebView2_19>() else {
            return Ok(());
        };
        let level = if low {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(1)
        } else {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(0)
        };
        unsafe { webview19.SetMemoryUsageTargetLevel(level)? };
        Ok(())
    }

    fn try_suspend(webview: &ICoreWebView2, label: &str) -> windows::core::Result<()> {
        let webview3 = webview.cast::<ICoreWebView2_3>()?;
        let trace_label = label.to_string();
        let handler = TrySuspendCompletedHandler::create(Box::new(move |result, success| {
            if result.is_err() || !success {
                debug!(label = %trace_label, "webview TrySuspend completed without success");
            } else {
                debug!(label = %trace_label, "webview TrySuspend succeeded");
            }
            Ok(())
        }));
        unsafe { webview3.TrySuspend(&handler)? };
        debug!(label, "webview TrySuspend requested");
        Ok(())
    }
}

#[cfg(windows)]
pub use imp::apply_from_controller;

#[cfg(not(windows))]
use crate::webview_power::WebviewPowerAction;

#[cfg(not(windows))]
pub fn apply_from_controller(_controller: &(), _action: WebviewPowerAction, _label: &str) {}
