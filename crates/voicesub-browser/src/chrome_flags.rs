/// Chrome feature gates disabled for Web Speech stability (SST Appendix A).
pub const DISABLED_CHROME_FEATURES: &[&str] = &[
    "CalculateNativeWinOcclusion",
    "HighEfficiencyModeAvailable",
    "HeuristicMemorySaver",
    "IntensiveWakeUpThrottling",
    "GlobalMediaControls",
];

pub fn disabled_chrome_features_csv() -> String {
    DISABLED_CHROME_FEATURES.join(",")
}

/// Shared Chromium args for classic browser worker launch.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_features_match_roadmap_appendix_a() {
        let csv = disabled_chrome_features_csv();
        assert!(csv.contains("CalculateNativeWinOcclusion"));
        assert!(csv.contains("GlobalMediaControls"));
    }
}
