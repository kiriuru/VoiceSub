//! Chrome supervisor for Browser Speech worker (SST `browser_worker_launcher.py` port).

mod chrome_flags;
mod ecoqos;
mod gateway;
mod launch_config;
mod launch_stability;
mod launcher;
mod operational_fsm;
mod orphan_guard;
mod process_affinity;
mod profile_bloat_guard;
mod service;
mod trace;
mod webview2_memory;
mod webview2_messages;
mod webview2_runtime;
mod webview_power;

pub use chrome_flags::{
    BrowserChromeLaunchConfig, CHROME_ANTI_THROTTLE_FLAGS, CHROME_DISK_BLOAT_GUARD_FLAGS,
    DISABLED_CHROME_DISK_BLOAT_FEATURES, DISABLED_CHROME_FEATURES, FORBIDDEN_WORKER_LAUNCH_FLAGS,
    default_anti_throttle_args, default_chrome_launch_value, default_disabled_chrome_features,
    disabled_chrome_features_csv, finalize_chrome_launch_config,
};
pub use gateway::{BrowserAsrGateway, GatewayDiagnostics};
pub use launch_config::chrome_launch_from_config;
pub use launch_stability::apply_launch_stability_overrides;
pub use launcher::{
    BrowserLaunchError, BrowserWorkerLauncher, LaunchResult, browser_worker_launch_skipped,
};
pub use operational_fsm::{BrowserAsrOperationalFsm, BrowserOperationalPhase};
pub use orphan_guard::{clear_worker_pid, reap_orphan_worker, record_worker_pid};
pub use process_affinity::{apply_browser_worker_affinity, resolve_browser_worker_affinity_mask};
pub use service::{
    BrowserAsrDiagnostics, BrowserAsrService, IngestCallback, IngestedAsrUpdate, StatusCallback,
    WorkerLifecycleCallback,
};
pub use trace::{BrowserAsrLog, StructuredLogFn, structured_log_from_runtime_logger};
pub use webview_power::{WebviewActivity, WebviewPowerAction, WebviewRole, resolve_power_action};
pub use webview2_messages::{
    WebView2MissingDialogCopy, normalize_supported_ui_language, system_supported_ui_language,
    webview2_missing_dialog_copy,
};
pub use webview2_runtime::{WEBVIEW2_APP_GUID, WEBVIEW2_DOWNLOAD_URL, installed_webview2_version};

#[cfg(windows)]
pub use webview2_memory::apply_from_controller;
