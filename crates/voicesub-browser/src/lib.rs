//! Chrome supervisor for Browser Speech worker (SST `browser_worker_launcher.py` port).

mod chrome_flags;
mod launch_config;
mod launch_stability;
mod process_affinity;
mod profile_bloat_guard;
mod ecoqos;
mod gateway;
mod launcher;
mod operational_fsm;
mod service;
mod trace;
mod webview2_messages;
mod webview2_memory;
mod webview2_runtime;
mod webview_power;

pub use chrome_flags::{
    default_anti_throttle_args, default_chrome_launch_value, default_disabled_chrome_features,
    disabled_chrome_features_csv, finalize_chrome_launch_config, BrowserChromeLaunchConfig,
    CHROME_ANTI_THROTTLE_FLAGS, CHROME_DISK_BLOAT_GUARD_FLAGS, DISABLED_CHROME_DISK_BLOAT_FEATURES,
    DISABLED_CHROME_FEATURES, FORBIDDEN_WORKER_LAUNCH_FLAGS,
};
pub use launch_config::chrome_launch_from_config;
pub use launch_stability::apply_launch_stability_overrides;
pub use process_affinity::{apply_browser_worker_affinity, resolve_browser_worker_affinity_mask};
pub use launcher::{browser_worker_launch_skipped, BrowserLaunchError, BrowserWorkerLauncher, LaunchResult};
pub use gateway::{BrowserAsrGateway, GatewayDiagnostics};
pub use operational_fsm::{BrowserAsrOperationalFsm, BrowserOperationalPhase};
pub use trace::{structured_log_from_runtime_logger, BrowserAsrLog, StructuredLogFn};
pub use service::{
    BrowserAsrDiagnostics, BrowserAsrService, IngestCallback, IngestedAsrUpdate, StatusCallback,
    WorkerLifecycleCallback,
};
pub use webview2_messages::{
    normalize_supported_ui_language, system_supported_ui_language, webview2_missing_dialog_copy,
    WebView2MissingDialogCopy,
};
pub use webview2_runtime::{
    installed_webview2_version, WEBVIEW2_APP_GUID, WEBVIEW2_DOWNLOAD_URL,
};
pub use webview_power::{
    resolve_power_action, WebviewActivity, WebviewPowerAction, WebviewRole,
};

#[cfg(windows)]
pub use webview2_memory::apply_from_controller;
