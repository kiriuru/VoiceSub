//! Static ACL matrix checks — keep permission TOMLs aligned with least-privilege intent.
//!
//! These tests do not exercise the Tauri runtime; they guard against re-expanding
//! `allow-voicesub-ipc` with TTS domain commands or re-adding dead privileged IPC.

fn allowed_commands(toml: &str) -> Vec<&str> {
    let mut cmds = Vec::new();
    let mut in_allow = false;
    for line in toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("commands.allow") {
            in_allow = true;
            continue;
        }
        if !in_allow {
            continue;
        }
        if trimmed == "]" {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix('"')
            && let Some(name) = rest.strip_suffix("\",").or_else(|| rest.strip_suffix('"'))
        {
            cmds.push(name);
        }
    }
    cmds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_ipc_is_shell_only() {
        let toml = include_str!("../permissions/allow-voicesub-ipc.toml");
        let cmds = allowed_commands(toml);
        assert_eq!(
            cmds,
            [
                "get_loopback_api_token",
                "get_runtime_state_snapshot",
                "set_dashboard_layout",
                "tts_open_window",
                "local_asr_open_window",
                "open_external_https_url",
                "open_local_http_url",
            ]
        );
        for forbidden in [
            "launch_browser_worker",
            "voicesub_version",
            "tts_get_config",
            "tts_play_audio",
            "tts_stop_channel",
            "tts_twitch_connect",
            "tts_open_system_url",
        ] {
            assert!(
                !cmds.contains(&forbidden),
                "main ACL must not include {forbidden}"
            );
        }
    }

    #[test]
    fn tts_ipc_excludes_shell_url_openers_and_dead_commands() {
        let toml = include_str!("../permissions/allow-voicesub-tts-ipc.toml");
        let cmds = allowed_commands(toml);
        assert!(cmds.contains(&"tts_get_config"));
        assert!(cmds.contains(&"tts_open_system_url"));
        assert!(cmds.contains(&"get_loopback_api_token"));
        for forbidden in [
            "open_external_https_url",
            "open_local_http_url",
            "tts_open_window",
            "local_asr_open_window",
            "launch_browser_worker",
            "tts_play_audio",
            "tts_stop_channel",
            "voicesub_version",
            "set_dashboard_layout",
        ] {
            assert!(
                !cmds.contains(&forbidden),
                "tts ACL must not include {forbidden}"
            );
        }
    }

    #[test]
    fn local_asr_ipc_is_minimal() {
        let toml = include_str!("../permissions/allow-voicesub-local-asr-ipc.toml");
        let cmds = allowed_commands(toml);
        assert_eq!(cmds, ["get_loopback_api_token", "open_external_https_url"]);
    }

    #[test]
    fn capabilities_deny_frontend_event_emit() {
        for path in [
            include_str!("../capabilities/default.json"),
            include_str!("../capabilities/tts.json"),
            include_str!("../capabilities/local-asr.json"),
        ] {
            assert!(
                path.contains("core:event:deny-emit"),
                "capability must deny emit"
            );
            assert!(
                path.contains("core:event:deny-emit-to"),
                "capability must deny emit-to"
            );
            assert!(
                !path.contains("allow-create-webview-window"),
                "frontend must not create webview windows"
            );
            assert!(
                !path.contains("core:window:allow-create"),
                "frontend must not create windows"
            );
        }
    }

    /// Frontend `invoke` inventory (keep in sync with `src/`, `src-tts/`, `src-local-asr/`).
    /// Save config / diagnostics export use HTTP `/api/*` + loopback token — not Tauri ACL
    /// beyond `get_loopback_api_token`.
    #[test]
    fn frontend_invokes_are_allowlisted_per_window() {
        let main = allowed_commands(include_str!("../permissions/allow-voicesub-ipc.toml"));
        let tts = allowed_commands(include_str!("../permissions/allow-voicesub-tts-ipc.toml"));
        let local_asr =
            allowed_commands(include_str!("../permissions/allow-voicesub-local-asr-ipc.toml"));

        // Dashboard (`main`): module windows, layout, URLs, snapshot, token fallback.
        for cmd in [
            "get_loopback_api_token",
            "get_runtime_state_snapshot",
            "set_dashboard_layout",
            "tts_open_window",
            "local_asr_open_window",
            "open_external_https_url",
            "open_local_http_url",
        ] {
            assert!(main.contains(&cmd), "main ACL missing dashboard invoke {cmd}");
        }

        // TTS window: domain IPC + shared snapshot/token (runtime-events + loopback-api).
        for cmd in [
            "get_loopback_api_token",
            "get_runtime_state_snapshot",
            "tts_get_config",
            "tts_set_provider",
            "tts_set_enabled",
            "tts_set_audio_device",
            "tts_set_channel_audio_device",
            "tts_set_playback_mode",
            "tts_list_output_devices",
            "tts_get_audio_routing",
            "tts_bind_window_audio",
            "tts_update_speech_settings",
            "tts_update_voice_settings",
            "tts_reset_subtitle_planner",
            "tts_speak_sample",
            "tts_channel_clear",
            "tts_channel_force_idle",
            "tts_get_resource_telemetry",
            "tts_twitch_get_status",
            "tts_twitch_connect",
            "tts_twitch_disconnect",
            "tts_update_twitch_settings",
            "tts_open_system_url",
            "tts_report_webview_activity",
        ] {
            assert!(tts.contains(&cmd), "tts ACL missing TTS invoke {cmd}");
        }

        // Local ASR window: token fallback + driver/docs URLs (module API is HTTP).
        for cmd in ["get_loopback_api_token", "open_external_https_url"] {
            assert!(
                local_asr.contains(&cmd),
                "local-asr ACL missing module invoke {cmd}"
            );
        }
    }
}
