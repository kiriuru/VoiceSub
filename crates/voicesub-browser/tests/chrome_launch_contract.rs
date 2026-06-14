//! SST `test_desktop_browser_worker_launch.py` parity for Chrome worker flags.

use std::path::Path;

use voicesub_browser::{
    BrowserChromeLaunchConfig, BrowserWorkerLauncher, CHROME_ANTI_THROTTLE_FLAGS,
    CHROME_DISK_BLOAT_GUARD_FLAGS, DISABLED_CHROME_DISK_BLOAT_FEATURES, DISABLED_CHROME_FEATURES,
    FORBIDDEN_WORKER_LAUNCH_FLAGS, finalize_chrome_launch_config,
};
use voicesub_browser::chrome_launch_from_config;

fn flag_present(args: &[String], required: &str) -> bool {
    let key = required.split('=').next().unwrap_or(required);
    args.iter().any(|arg| arg == required || arg == key || arg.starts_with(&format!("{key}=")))
}

#[test]
fn sst_required_launch_flags_present_in_default_config() {
    let config = BrowserChromeLaunchConfig::default();
    for required in CHROME_ANTI_THROTTLE_FLAGS {
        assert!(
            flag_present(&config.launch_args, required),
            "missing default launch flag {required}"
        );
    }
}

#[test]
fn sst_disabled_features_present_in_default_config() {
    let config = BrowserChromeLaunchConfig::default();
    for feature in DISABLED_CHROME_FEATURES {
        assert!(
            config.disabled_features.iter().any(|f| f == feature),
            "missing default disabled feature {feature}"
        );
    }
}

#[test]
fn partial_config_merge_restores_full_sst_flag_set() {
    let cfg = chrome_launch_from_config(&serde_json::json!({
        "asr": {
            "browser": {
                "chrome_launch": {
                    "launch_args": ["--new-window"],
                    "disabled_features": ["GlobalMediaControls"]
                }
            }
        }
    }));
    for required in CHROME_ANTI_THROTTLE_FLAGS {
        assert!(
            flag_present(&cfg.launch_args, required),
            "partial config missing launch flag {required}"
        );
    }
    for feature in DISABLED_CHROME_FEATURES {
        assert!(
            cfg.disabled_features.iter().any(|f| f == feature),
            "partial config missing disabled feature {feature}"
        );
    }
}

#[test]
fn launch_argv_matches_sst_worker_window_contract() {
    let config = BrowserChromeLaunchConfig::default();
    let chrome = Path::new(r"C:\Program Files\Google\Chrome\Application\chrome.exe");
    let launcher = BrowserWorkerLauncher::new("user-data");
    let profile = launcher.profile_dir("http://127.0.0.1:8765/google-asr?autostart=1", chrome);
    let args = config.launch_args_for_url(&profile, "http://127.0.0.1:8765/google-asr?autostart=1");

    for required in CHROME_ANTI_THROTTLE_FLAGS {
        assert!(
            flag_present(&args, required),
            "launch argv missing {required}"
        );
    }
    assert!(args.iter().any(|a| a.starts_with("--user-data-dir=")));
    let disable_features = args
        .iter()
        .find(|a| a.starts_with("--disable-features="))
        .expect("--disable-features");
    for feature in DISABLED_CHROME_FEATURES {
        assert!(
            disable_features.contains(feature),
            "disable-features missing {feature}"
        );
    }
    for forbidden in FORBIDDEN_WORKER_LAUNCH_FLAGS {
        assert!(
            !args.iter().any(|a| a.starts_with(forbidden)),
            "forbidden worker flag present: {forbidden}"
        );
    }
    assert!(args.last().is_some_and(|url| url.contains("/google-asr")));
}

#[test]
fn disk_bloat_guard_flags_present_in_default_config() {
    let config = BrowserChromeLaunchConfig::default();
    for required in CHROME_DISK_BLOAT_GUARD_FLAGS {
        assert!(
            flag_present(&config.launch_args, required),
            "missing disk bloat guard launch flag {required}"
        );
    }
    for feature in DISABLED_CHROME_DISK_BLOAT_FEATURES {
        assert!(
            config.disabled_features.iter().any(|f| f == feature),
            "missing disk bloat guard disabled feature {feature}"
        );
    }
}

#[test]
fn launch_argv_includes_disk_bloat_guard() {
    let config = BrowserChromeLaunchConfig::default();
    let chrome = Path::new(r"C:\Program Files\Google\Chrome\Application\chrome.exe");
    let launcher = BrowserWorkerLauncher::new("user-data");
    let profile = launcher.profile_dir("http://127.0.0.1:8765/google-asr?autostart=1", chrome);
    let args = config.launch_args_for_url(&profile, "http://127.0.0.1:8765/google-asr?autostart=1");

    for required in CHROME_DISK_BLOAT_GUARD_FLAGS {
        assert!(
            flag_present(&args, required),
            "launch argv missing disk bloat guard flag {required}"
        );
    }
    let disable_features = args
        .iter()
        .find(|a| a.starts_with("--disable-features="))
        .expect("--disable-features");
    for feature in DISABLED_CHROME_DISK_BLOAT_FEATURES {
        assert!(
            disable_features.contains(feature),
            "disable-features missing disk bloat guard feature {feature}"
        );
    }
}

#[test]
fn high_priority_defaults_on_like_sst() {
    let config = BrowserChromeLaunchConfig::default();
    assert!(config.use_high_priority);
}

#[test]
fn finalize_strips_forbidden_flags_from_extra_args() {
    let mut config = BrowserChromeLaunchConfig {
        launch_args: BrowserChromeLaunchConfig::default().launch_args,
        disabled_features: voicesub_browser::default_disabled_chrome_features(),
        extra_args: vec!["--disable-extensions".to_string(), "--bwsi".to_string()],
        use_high_priority: true,
    };
    finalize_chrome_launch_config(&mut config);
    assert!(config.extra_args.is_empty());
}
