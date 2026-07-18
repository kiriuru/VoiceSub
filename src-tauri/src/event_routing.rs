//! Per-window routing for `runtime-event` IPC envelopes.
//!
//! Historically every runtime event was emitted globally to **all** webviews. During
//! active speech the high-frequency `transcript_update` / `overlay_update` stream then
//! flooded the TTS webview's IPC channel even though it discards those types, saturating
//! the WebView2 UI-thread message loop (review §1). Routing keeps the main dashboard as
//! the full consumer while the TTS window receives only the handful of types it acts on.

use serde_json::{Value, json};
use voicesub_runtime::RuntimeStateSnapshot;

pub const MAIN_WINDOW_LABEL: &str = "main";

/// Event types the TTS webview actually consumes (`src-tts/App.svelte::handleRuntimeEvent`).
/// Everything else is dashboard-only and must not be emitted to the TTS window.
pub fn tts_window_wants(event_type: &str) -> bool {
    matches!(
        event_type,
        "twitch_chat_message"
            | "twitch_connection_update"
            | "runtime_update"
            | "runtime_status"
            | "ui_config_sync"
    )
}

/// Local ASR module only needs live UI presentation sync (theme/locale/font/palette).
pub fn local_asr_window_wants(event_type: &str) -> bool {
    matches!(event_type, "ui_config_sync")
}

/// Rebuild `{ type, payload }` envelopes from a runtime snapshot so a lagged IPC
/// consumer can re-synchronize after dropping broadcast messages (review §9).
///
/// Prefer `overlay_update` (live shape) over raw `subtitle_payload_update` so the
/// dashboard store is not double-applied with two equivalent presentation bodies.
pub fn snapshot_to_envelopes(snapshot: &RuntimeStateSnapshot) -> Vec<Value> {
    let mut envelopes = Vec::new();
    let mut push = |event_type: &str, payload: Option<&Value>| {
        let payload = match payload {
            Some(value) if value.is_object() => value.clone(),
            _ => return,
        };
        envelopes.push(json!({ "type": event_type, "payload": payload }));
    };
    if snapshot.runtime.is_object() {
        push("runtime_update", Some(&snapshot.runtime));
    }
    if snapshot
        .overlay
        .as_ref()
        .is_some_and(|value| value.is_object())
    {
        push("overlay_update", snapshot.overlay.as_ref());
    } else {
        push("subtitle_payload_update", snapshot.subtitle.as_ref());
    }
    push("translation_update", snapshot.translation.as_ref());
    push("diagnostics_update", snapshot.diagnostics.as_ref());
    // Twitch connection status is consumed by the TTS window; replay it so its connection
    // UI is restored after a bus lag (review MED#7).
    push(
        "twitch_connection_update",
        snapshot.twitch_connection.as_ref(),
    );
    envelopes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tts_window_only_wants_its_event_types() {
        assert!(tts_window_wants("twitch_chat_message"));
        assert!(tts_window_wants("runtime_update"));
        assert!(tts_window_wants("ui_config_sync"));
        // High-frequency hot-path types must stay out of the TTS window.
        assert!(!tts_window_wants("transcript_update"));
        assert!(!tts_window_wants("overlay_update"));
        assert!(!tts_window_wants("translation_update"));
        assert!(!tts_window_wants("diagnostics_update"));
    }

    #[test]
    fn local_asr_window_only_wants_ui_config_sync() {
        assert!(local_asr_window_wants("ui_config_sync"));
        assert!(!local_asr_window_wants("runtime_update"));
        assert!(!local_asr_window_wants("transcript_update"));
        assert!(!local_asr_window_wants("overlay_update"));
    }

    #[test]
    fn snapshot_envelopes_skip_missing_and_non_object_fields() {
        let snapshot = RuntimeStateSnapshot {
            rev: 7,
            runtime: json!({ "running": true }),
            subtitle: None,
            overlay: Some(json!({ "lifecycle_state": "partial_only" })),
            translation: Some(Value::Null),
            diagnostics: None,
            twitch_connection: None,
        };
        let envelopes = snapshot_to_envelopes(&snapshot);
        let types: Vec<&str> = envelopes
            .iter()
            .filter_map(|e| e.get("type").and_then(Value::as_str))
            .collect();
        assert_eq!(types, vec!["runtime_update", "overlay_update"]);
    }

    #[test]
    fn snapshot_envelopes_prefer_overlay_over_subtitle() {
        let snapshot = RuntimeStateSnapshot {
            rev: 8,
            runtime: json!({ "running": true }),
            subtitle: Some(json!({ "sequence": 1, "raw": true })),
            overlay: Some(json!({ "sequence": 1, "lifecycle_state": "partial_only" })),
            translation: None,
            diagnostics: None,
            twitch_connection: None,
        };
        let envelopes = snapshot_to_envelopes(&snapshot);
        let types: Vec<&str> = envelopes
            .iter()
            .filter_map(|e| e.get("type").and_then(Value::as_str))
            .collect();
        assert_eq!(types, vec!["runtime_update", "overlay_update"]);
        assert!(!types.contains(&"subtitle_payload_update"));
    }

    #[test]
    fn snapshot_envelopes_fall_back_to_subtitle_when_overlay_missing() {
        let snapshot = RuntimeStateSnapshot {
            rev: 9,
            runtime: json!({ "running": true }),
            subtitle: Some(json!({ "sequence": 2 })),
            overlay: None,
            translation: None,
            diagnostics: None,
            twitch_connection: None,
        };
        let envelopes = snapshot_to_envelopes(&snapshot);
        let types: Vec<&str> = envelopes
            .iter()
            .filter_map(|e| e.get("type").and_then(Value::as_str))
            .collect();
        assert_eq!(types, vec!["runtime_update", "subtitle_payload_update"]);
    }

    #[test]
    fn snapshot_envelopes_replay_twitch_connection_for_tts_window() {
        let snapshot = RuntimeStateSnapshot {
            rev: 11,
            runtime: json!({ "running": false }),
            subtitle: None,
            overlay: None,
            translation: None,
            diagnostics: None,
            twitch_connection: Some(json!({ "state": "connected", "channel": "demo" })),
        };
        let envelopes = snapshot_to_envelopes(&snapshot);
        let types: Vec<&str> = envelopes
            .iter()
            .filter_map(|e| e.get("type").and_then(Value::as_str))
            .collect();
        assert_eq!(types, vec!["runtime_update", "twitch_connection_update"]);
        // The replayed type must be one the TTS window actually consumes.
        assert!(tts_window_wants("twitch_connection_update"));
    }
}
