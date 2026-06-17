//! Optional runtime launch overrides for long-stream stability (env / config profile).
//!
//! SST defaults (`use_high_priority: true`, no `--disable-gpu`) stay in config seed;
//! these overrides are opt-in so golden parity tests remain unchanged.

use crate::chrome_flags::BrowserChromeLaunchConfig;

const ENV_STABILITY: &str = "VOICESUB_BROWSER_STABILITY";
const ENV_HIGH_PRIORITY: &str = "VOICESUB_BROWSER_HIGH_PRIORITY";
const ENV_DISABLE_GPU: &str = "VOICESUB_BROWSER_DISABLE_GPU";

fn env_truthy(name: &str) -> Option<bool> {
    match std::env::var(name).ok().as_deref() {
        Some("1") | Some("true") | Some("yes") | Some("on") => Some(true),
        Some("0") | Some("false") | Some("no") | Some("off") => Some(false),
        _ => None,
    }
}

fn ensure_flag(args: &mut Vec<String>, flag: &str) {
    if !args
        .iter()
        .any(|arg| arg == flag || arg.starts_with(&format!("{flag}=")))
    {
        args.push(flag.to_string());
    }
}

/// Apply `asr.browser.chrome_launch.stability_profile` and/or env overrides.
pub fn apply_launch_stability_overrides(
    config: &mut BrowserChromeLaunchConfig,
    stability_profile: bool,
) {
    let stability = stability_profile || env_truthy(ENV_STABILITY) == Some(true);

    if stability {
        config.use_high_priority = false;
        ensure_flag(&mut config.extra_args, "--disable-gpu");
    }

    if let Some(high_priority) = env_truthy(ENV_HIGH_PRIORITY) {
        config.use_high_priority = high_priority;
    }

    if env_truthy(ENV_DISABLE_GPU) == Some(true) {
        ensure_flag(&mut config.extra_args, "--disable-gpu");
    } else if env_truthy(ENV_DISABLE_GPU) == Some(false) {
        config.extra_args.retain(|arg| arg != "--disable-gpu");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome_flags::BrowserChromeLaunchConfig;

    fn clear_stability_env() {
        unsafe {
            std::env::remove_var(ENV_STABILITY);
            std::env::remove_var(ENV_HIGH_PRIORITY);
            std::env::remove_var(ENV_DISABLE_GPU);
        }
    }

    #[test]
    fn launch_stability_override_cases() {
        clear_stability_env();

        let mut config = BrowserChromeLaunchConfig::default();
        apply_launch_stability_overrides(&mut config, true);
        assert!(!config.use_high_priority);
        assert!(config.extra_args.iter().any(|arg| arg == "--disable-gpu"));

        clear_stability_env();
        let mut config = BrowserChromeLaunchConfig::default();
        unsafe {
            std::env::set_var(ENV_HIGH_PRIORITY, "0");
        }
        apply_launch_stability_overrides(&mut config, false);
        assert!(!config.use_high_priority);

        clear_stability_env();
        let mut config = BrowserChromeLaunchConfig::default();
        unsafe {
            std::env::set_var(ENV_DISABLE_GPU, "1");
        }
        apply_launch_stability_overrides(&mut config, false);
        assert!(config.use_high_priority);
        assert!(config.extra_args.iter().any(|arg| arg == "--disable-gpu"));

        clear_stability_env();
    }
}
