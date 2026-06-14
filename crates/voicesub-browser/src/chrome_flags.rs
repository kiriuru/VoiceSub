use serde_json::{json, Value};

/// Chrome feature gates disabled for Web Speech stability (SST `browser_worker_launcher.py`).
pub const DISABLED_CHROME_FEATURES: &[&str] = &[
    "CalculateNativeWinOcclusion",
    "HighEfficiencyModeAvailable",
    "HeuristicMemorySaver",
    "IntensiveWakeUpThrottling",
    "GlobalMediaControls",
];

/// Feature gates that block multi-GB Optimization Guide / Gemini Nano downloads in the
/// isolated worker profile (Chrome 147+; not present in SST 0.4.4 era).
pub const DISABLED_CHROME_DISK_BLOAT_FEATURES: &[&str] = &[
    "OptimizationGuideModelDownloading",
    "OptimizationGuideOnDeviceModel",
    "OptimizationHints",
    "OptimizationHintsFetching",
    "OnDeviceModelBackgroundDownload",
];

/// Launch flags that keep the worker profile from accumulating component CRX / sync data.
pub const CHROME_DISK_BLOAT_GUARD_FLAGS: &[&str] = &[
    "--disable-component-update",
    "--disable-sync",
];

/// Launch flags always applied for the classic Browser Speech worker (SST §11.1 / Appendix A).
pub const CHROME_ANTI_THROTTLE_FLAGS: &[&str] = &[
    "--new-window",
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-default-apps",
    "--disable-session-crashed-bubble",
    "--disable-backgrounding-occluded-windows",
    "--disable-renderer-backgrounding",
    "--disable-background-timer-throttling",
    "--noerrdialogs",
    "--window-size=980,860",
];

/// Worker launch must never use kiosk/hidden-profile flags (SST contract tests).
pub const FORBIDDEN_WORKER_LAUNCH_FLAGS: &[&str] = &["--app=", "--disable-extensions", "--bwsi"];

pub fn disabled_chrome_features_csv() -> String {
    DISABLED_CHROME_FEATURES.join(",")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserChromeLaunchConfig {
    pub launch_args: Vec<String>,
    pub disabled_features: Vec<String>,
    pub extra_args: Vec<String>,
    pub use_high_priority: bool,
}

impl Default for BrowserChromeLaunchConfig {
    fn default() -> Self {
        Self {
            launch_args: default_anti_throttle_args(),
            disabled_features: default_disabled_chrome_features(),
            extra_args: Vec::new(),
            use_high_priority: true,
        }
    }
}

impl BrowserChromeLaunchConfig {
    pub fn disabled_features_csv(&self) -> String {
        self.disabled_features.join(",")
    }

    pub fn launch_args_for_url(&self, profile_dir: &std::path::Path, worker_url: &str) -> Vec<String> {
        let mut config = self.clone();
        finalize_chrome_launch_config(&mut config);

        let disabled_features = config.disabled_features_csv();
        let mut args = config.launch_args;
        args.extend(config.extra_args);
        strip_launch_time_only_flags(&mut args);
        args.push(format!("--user-data-dir={}", profile_dir.display()));
        args.push(format!("--disable-features={disabled_features}"));
        args.push(worker_url.to_string());
        args
    }
}

pub fn default_anti_throttle_args() -> Vec<String> {
    CHROME_ANTI_THROTTLE_FLAGS
        .iter()
        .chain(CHROME_DISK_BLOAT_GUARD_FLAGS.iter())
        .map(|flag| (*flag).to_string())
        .collect()
}

pub fn default_disabled_chrome_features() -> Vec<String> {
    DISABLED_CHROME_FEATURES
        .iter()
        .chain(DISABLED_CHROME_DISK_BLOAT_FEATURES.iter())
        .map(|feature| (*feature).to_string())
        .collect()
}

/// Merge SST-required launch flags and disabled features; strip forbidden args.
pub fn finalize_chrome_launch_config(config: &mut BrowserChromeLaunchConfig) {
    ensure_sst_launch_flags(&mut config.launch_args);
    ensure_disk_bloat_guard_flags(&mut config.launch_args);
    ensure_sst_disabled_features(&mut config.disabled_features);
    ensure_disk_bloat_guard_features(&mut config.disabled_features);
    sanitize_forbidden_launch_args(&mut config.launch_args);
    sanitize_forbidden_launch_args(&mut config.extra_args);
}

fn flag_present(args: &[String], required: &str) -> bool {
    let key = required.split('=').next().unwrap_or(required);
    args.iter().any(|arg| arg == required || arg == key || arg.starts_with(&format!("{key}=")))
}

pub fn ensure_sst_launch_flags(args: &mut Vec<String>) {
    for required in CHROME_ANTI_THROTTLE_FLAGS {
        if !flag_present(args, required) {
            args.push((*required).to_string());
        }
    }
}

pub fn ensure_sst_disabled_features(features: &mut Vec<String>) {
    for required in DISABLED_CHROME_FEATURES {
        if !features.iter().any(|feature| feature == required) {
            features.push((*required).to_string());
        }
    }
}

pub fn ensure_disk_bloat_guard_flags(args: &mut Vec<String>) {
    for required in CHROME_DISK_BLOAT_GUARD_FLAGS {
        if !flag_present(args, required) {
            args.push((*required).to_string());
        }
    }
}

pub fn ensure_disk_bloat_guard_features(features: &mut Vec<String>) {
    for required in DISABLED_CHROME_DISK_BLOAT_FEATURES {
        if !features.iter().any(|feature| feature == required) {
            features.push((*required).to_string());
        }
    }
}

pub fn sanitize_forbidden_launch_args(args: &mut Vec<String>) {
    args.retain(|arg| {
        !FORBIDDEN_WORKER_LAUNCH_FLAGS
            .iter()
            .any(|forbidden| arg.starts_with(forbidden))
    });
}

fn strip_launch_time_only_flags(args: &mut Vec<String>) {
    args.retain(|arg| {
        !arg.starts_with("--user-data-dir=") && !arg.starts_with("--disable-features=")
    });
}

/// Default `asr.browser.chrome_launch` object for config seeding / normalization.
pub fn default_chrome_launch_value() -> Value {
    json!({
        "launch_args": default_anti_throttle_args(),
        "disabled_features": default_disabled_chrome_features(),
        "extra_args": [],
        "use_high_priority": true
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_features_match_sst_launcher() {
        let csv = disabled_chrome_features_csv();
        for feature in DISABLED_CHROME_FEATURES {
            assert!(csv.contains(feature), "missing disabled feature {feature}");
        }
    }

    #[test]
    fn partial_launch_args_are_merged_with_sst_defaults() {
        let mut config = BrowserChromeLaunchConfig {
            launch_args: vec!["--new-window".to_string()],
            disabled_features: vec!["GlobalMediaControls".to_string()],
            extra_args: Vec::new(),
            use_high_priority: true,
        };
        finalize_chrome_launch_config(&mut config);
        for required in CHROME_ANTI_THROTTLE_FLAGS {
            assert!(
                flag_present(&config.launch_args, required),
                "missing merged launch flag {required}"
            );
        }
        for required in DISABLED_CHROME_FEATURES {
            assert!(
                config.disabled_features.iter().any(|f| f == required),
                "missing merged disabled feature {required}"
            );
        }
    }

    #[test]
    fn disk_bloat_guard_is_merged_into_partial_config() {
        let mut config = BrowserChromeLaunchConfig {
            launch_args: vec!["--new-window".to_string()],
            disabled_features: vec!["GlobalMediaControls".to_string()],
            extra_args: Vec::new(),
            use_high_priority: true,
        };
        finalize_chrome_launch_config(&mut config);
        for required in CHROME_DISK_BLOAT_GUARD_FLAGS {
            assert!(
                flag_present(&config.launch_args, required),
                "missing merged disk bloat guard flag {required}"
            );
        }
        for required in DISABLED_CHROME_DISK_BLOAT_FEATURES {
            assert!(
                config.disabled_features.iter().any(|f| f == required),
                "missing merged disk bloat guard feature {required}"
            );
        }
    }

    #[test]
    fn forbidden_worker_flags_are_stripped() {
        let mut args = vec![
            "--new-window".to_string(),
            "--disable-extensions".to_string(),
            "--app=http://evil".to_string(),
        ];
        sanitize_forbidden_launch_args(&mut args);
        assert_eq!(args, vec!["--new-window".to_string()]);
    }

    #[test]
    fn launch_args_for_url_includes_profile_features_and_url() {
        let config = BrowserChromeLaunchConfig::default();
        let args = config.launch_args_for_url(
            std::path::Path::new(r"C:\VoiceSub\user-data\browser-worker-profile-classic-chrome"),
            "http://127.0.0.1:8765/google-asr?autostart=1",
        );
        let joined = args.join(" ");
        for required in CHROME_ANTI_THROTTLE_FLAGS {
            assert!(
                flag_present(&args, required),
                "launch argv missing {required}: {joined}"
            );
        }
        assert!(args.iter().any(|a| a.starts_with("--user-data-dir=")));
        assert!(args.iter().any(|a| a.starts_with("--disable-features=")));
        assert!(args.last().is_some_and(|url| url.contains("/google-asr")));
        for forbidden in FORBIDDEN_WORKER_LAUNCH_FLAGS {
            assert!(
                !args.iter().any(|a| a.starts_with(forbidden)),
                "forbidden flag present: {forbidden}"
            );
        }
    }
}
