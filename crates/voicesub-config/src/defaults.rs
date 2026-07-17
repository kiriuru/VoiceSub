use serde_json::{Value, json};
use voicesub_types::DEFAULT_GITHUB_REPO;

use crate::translation_normalize::default_translation_provider_settings;

pub const CURRENT_CONFIG_VERSION: i64 = 8;

pub const ASR_MODE_BROWSER: &str = "browser_google";
pub const ASR_MODE_LOCAL_PARAKEET: &str = "local_parakeet";

/// VoiceSub 0.5.0 default — `browser_google` only.
pub fn default_config_payload() -> Value {
    json!({
        "config_version": CURRENT_CONFIG_VERSION,
        "profile": "default",
        "ui": {
            "language": "",
            "layout": "standard",
            "show_translation_results": true,
            "theme": "dark",
            "palette": {
                "accent": "#6cc7ff",
                "accent_secondary": "#ff6ce6",
                "accent_tertiary": "#7ce3ad"
            }
        },
        "source_lang": "auto",
        "targets": ["en"],
        "asr": {
            "mode": "browser_google",
            "browser": {
                "recognition_language": "en-US",
                "worker_launch_browser": "auto",
                "interim_results": true,
                "continuous_results": true,
                "force_finalization_enabled": true,
                "force_finalization_timeout_ms": 1600,
                "minimum_reconnect_interval_ms": 500,
                "normal_restart_delay_ms": 150,
                "no_speech_restart_delay_ms": 150,
                "network_reconnect_initial_ms": 500,
                "network_reconnect_max_ms": 30000,
                "stuck_stopping_timeout_ms": 2000,
                "max_browser_session_age_ms": 180000,
                "prepare_cycle_before_ms": 30000,
                "force_final_on_interruption": true,
                "force_final_min_chars": 8,
                "force_final_min_stable_ms": 750,
                "chrome_launch": {
                    "launch_args": [
                        "--new-window",
                        "--no-first-run",
                        "--no-default-browser-check",
                        "--disable-default-apps",
                        "--disable-session-crashed-bubble",
                        "--disable-backgrounding-occluded-windows",
                        "--disable-renderer-backgrounding",
                        "--disable-background-timer-throttling",
                        "--noerrdialogs",
                        "--window-size=980,860"
                    ],
                    "disabled_features": [
                        "CalculateNativeWinOcclusion",
                        "HighEfficiencyModeAvailable",
                        "HeuristicMemorySaver",
                        "IntensiveWakeUpThrottling",
                        "GlobalMediaControls"
                    ],
                    "extra_args": [],
                    "use_high_priority": true
                }
            }
        },
        "overlay": {
            "preset": "single",
            "compact": false
        },
        "obs_closed_captions": {
            "enabled": false,
            "output_mode": "disabled",
            "connection": {
                "host": "127.0.0.1",
                "port": 4455,
                "password": ""
            },
            "debug_mirror": {
                "enabled": false,
                "input_name": "CC_DEBUG",
                "send_partials": true
            },
            "timing": {
                "send_partials": true,
                "partial_throttle_ms": 140,
                "min_partial_delta_chars": 1,
                "final_replace_delay_ms": 0,
                "clear_after_ms": 2500,
                "avoid_duplicate_text": true
            }
        },
        "translation": {
            "enabled": false,
            "provider": "google_translate_v2",
            "target_languages": ["en"],
            "lines": [{
                "slot_id": "translation_1",
                "enabled": true,
                "target_lang": "en",
                "provider": "google_translate_v2",
                "label": "EN"
            }],
            "timeout_ms": 10000,
            "queue_max_size": 8,
            "max_concurrent_jobs": 2,
            "cache": {
                "enabled": true,
                "persist": true,
                "max_entries": 5000
            },
            "provider_limits": {},
            "provider_settings": default_translation_provider_settings()
        },
        "subtitle_output": {
            "show_source": true,
            "show_translations": true,
            "max_translation_languages": 2,
            "display_order": ["source", "translation_1"]
        },
        "subtitle_lifecycle": {
            "completed_block_ttl_ms": 4500,
            "completed_source_ttl_ms": 4500,
            "completed_translation_ttl_ms": 4500,
            // Deprecated: normalized for legacy configs; no runtime effect (see normalize_subtitle_lifecycle).
            "pause_to_finalize_ms": 350,
            "allow_early_replace_on_next_final": true,
            "sync_source_and_translation_expiry": true,
            "keep_completed_translation_during_active_partial": true,
            // Deprecated: normalized for legacy configs; no runtime effect.
            "hard_max_phrase_ms": 5500
        },
        "source_text_replacement": {
            "enabled": false,
            "include_builtin": true,
            "case_insensitive": true,
            "whole_words": true,
            "pairs": []
        },
        "logging": {
            "full_enabled": false
        },
        "updates": {
            "enabled": true,
            "provider": "github_releases",
            "github_repo": DEFAULT_GITHUB_REPO,
            "release_channel": "stable",
            "check_interval_hours": 12,
            "last_checked_utc": "",
            "latest_known_version": ""
        }
    })
}
