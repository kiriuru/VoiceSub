use serde_json::Value;

use crate::chrome_flags::{
    BrowserChromeLaunchConfig, default_anti_throttle_args, default_disabled_chrome_features,
    finalize_chrome_launch_config,
};
use crate::launch_stability::apply_launch_stability_overrides;

/// Read `asr.browser.chrome_launch` from config; missing keys use SST Appendix A defaults.
pub fn chrome_launch_from_config(config: &Value) -> BrowserChromeLaunchConfig {
    let browser = config
        .get("asr")
        .and_then(|v| v.get("browser"))
        .and_then(Value::as_object);
    let Some(browser) = browser else {
        return BrowserChromeLaunchConfig::default();
    };
    let chrome = browser.get("chrome_launch").and_then(Value::as_object);
    let Some(chrome) = chrome else {
        return BrowserChromeLaunchConfig::default();
    };

    let launch_args = string_array(chrome.get("launch_args"))
        .filter(|items| !items.is_empty())
        .unwrap_or_else(default_anti_throttle_args);
    let disabled_features = string_array(chrome.get("disabled_features"))
        .filter(|items| !items.is_empty())
        .unwrap_or_else(default_disabled_chrome_features);
    let extra_args = string_array(chrome.get("extra_args")).unwrap_or_default();
    let use_high_priority = chrome
        .get("use_high_priority")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let stability_profile = chrome
        .get("stability_profile")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let mut config = BrowserChromeLaunchConfig {
        launch_args,
        disabled_features,
        extra_args,
        use_high_priority,
    };
    finalize_chrome_launch_config(&mut config);
    apply_launch_stability_overrides(&mut config, stability_profile);
    config
}

fn string_array(value: Option<&Value>) -> Option<Vec<String>> {
    let items = value?.as_array()?;
    let parsed: Vec<String> = items
        .iter()
        .filter_map(|item| {
            item.as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .collect();
    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn uses_defaults_when_section_missing() {
        let cfg = chrome_launch_from_config(&json!({ "asr": { "browser": {} } }));
        assert!(cfg.use_high_priority);
        assert!(
            cfg.launch_args
                .contains(&"--disable-background-timer-throttling".to_string())
        );
    }

    #[test]
    fn reads_extra_args_from_config() {
        let cfg = chrome_launch_from_config(&json!({
            "asr": {
                "browser": {
                    "chrome_launch": {
                        "extra_args": ["--mute-audio"],
                        "use_high_priority": false
                    }
                }
            }
        }));
        assert!(!cfg.use_high_priority);
        assert!(cfg.extra_args.contains(&"--mute-audio".to_string()));
    }

    #[test]
    fn partial_config_from_json_is_merged_with_sst_defaults() {
        let cfg = chrome_launch_from_config(&json!({
            "asr": {
                "browser": {
                    "chrome_launch": {
                        "launch_args": ["--new-window"],
                        "disabled_features": ["GlobalMediaControls"]
                    }
                }
            }
        }));
        assert!(
            cfg.launch_args
                .contains(&"--disable-background-timer-throttling".to_string())
        );
        assert!(
            cfg.disabled_features
                .iter()
                .any(|f| f == "CalculateNativeWinOcclusion")
        );
    }

    #[test]
    fn stability_profile_from_config() {
        unsafe {
            std::env::remove_var("VOICESUB_BROWSER_STABILITY");
            std::env::remove_var("VOICESUB_BROWSER_HIGH_PRIORITY");
            std::env::remove_var("VOICESUB_BROWSER_DISABLE_GPU");
        }
        let cfg = chrome_launch_from_config(&json!({
            "asr": {
                "browser": {
                    "chrome_launch": {
                        "stability_profile": true
                    }
                }
            }
        }));
        assert!(!cfg.use_high_priority);
        assert!(cfg.extra_args.iter().any(|arg| arg == "--disable-gpu"));
    }
}
