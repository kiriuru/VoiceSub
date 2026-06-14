use std::collections::BTreeMap;

use chrono::Local;
use serde_json::Value;

/// Map structured runtime event names to compact severity labels (SST parity).
///
/// Default disk footprint (`should_write_runtime_event` with `verbose=false`) keeps
/// only `INF` / `WRN` / `ERR` / `CRT`. SST 0.4.4 lists four `INF` events and two
/// `WRN` events; everything else is `DBG` unless explicitly marked `VRB` for
/// high-frequency heartbeats that must stay out of compact mode.
pub fn structured_event_level(event: &str) -> &'static str {
    let ev = event.trim().to_ascii_lowercase();
    match ev.as_str() {
        "browser_worker_status"
        | "browser_rearm_scheduled"
        | "translation_queue_depth_changed"
        | "runtime_status_duplicate_suppressed"
        | "runtime_status_heartbeat_sent"
        | "runtime_status_broadcast"
        | "runtime_events_duplicate_suppressed"
        | "asr_ingest_partial_suppressed"
        | "ws_outbound_queue_pressure"
        | "ws_outbound_queue_drop_oldest" => "VRB",
        "translation_job_error"
        | "translation_line_error"
        | "obs_caption_send_failed" => "ERR",
        "browser_onerror" | "browser_degraded" => "WRN",
        "translation_publish_accepted"
        | "browser_external_final"
        | "browser_overlap_handoff"
        | "browser_overlap_buddy_ghost_recovered"
        | "diagnostics_bundle_exported"
        | "browser_worker_disconnected" => "INF",
        _ => "DBG",
    }
}

pub fn default_runtime_levels() -> &'static [&'static str] {
    &["INF", "WRN", "ERR", "CRT"]
}

pub fn should_write_runtime_event(event: &str, verbose: bool) -> bool {
    let level = structured_event_level(event);
    verbose || default_runtime_levels().contains(&level)
}

fn truncate_utf8_prefix(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

pub(crate) fn truncate_with_ellipsis(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let ellipsis = '…';
    let budget = max_bytes.saturating_sub(ellipsis.len_utf8());
    if budget == 0 {
        return ellipsis.to_string();
    }
    format!("{}…", truncate_utf8_prefix(text, budget))
}

fn format_value(key: &str, value: &Value, max_len: usize) -> String {
    let raw = match value {
        Value::String(text) => text.replace('\n', "\\n"),
        other => other.to_string().replace('\n', "\\n"),
    };
    let raw = truncate_with_ellipsis(&raw, max_len);
    if raw.is_empty() || raw.contains(char::is_whitespace) || raw.contains('"') || raw.contains('=')
    {
        let esc = raw.replace('\\', "\\\\").replace('"', "\\\"");
        format!("{key}=\"{esc}\"")
    } else {
        format!("{key}={raw}")
    }
}

pub fn format_structured_runtime_line(
    event: &str,
    channel: &str,
    source: Option<&str>,
    fields: &BTreeMap<String, Value>,
) -> String {
    let ts = Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();
    let component_key = source
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(channel)
        .replace('_', " ");
    let component_display = component_key
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let lvl = structured_event_level(event);
    let mut parts = Vec::new();
    for (key, value) in fields {
        if value.is_null() {
            continue;
        }
        parts.push(format_value(key, value, 200));
    }
    parts.sort();
    let detail = parts.join(" ");
    if detail.is_empty() {
        format!("[{ts} {lvl}] {component_display} :: {event}")
    } else {
        format!("[{ts} {lvl}] {component_display} :: {event} {detail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_runtime_ws_events() {
        assert_eq!(structured_event_level("ws_connection_open"), "DBG");
        assert_eq!(structured_event_level("runtime_status_duplicate_suppressed"), "VRB");
        assert_eq!(structured_event_level("runtime_status_broadcast"), "VRB");
        assert_eq!(structured_event_level("asr_ingest_final_published"), "DBG");
    }

    #[test]
    fn classifies_translation_events() {
        assert_eq!(structured_event_level("translation_queue_depth_changed"), "VRB");
        assert_eq!(structured_event_level("translation_publish_accepted"), "INF");
        assert_eq!(structured_event_level("translation_line_error"), "ERR");
        assert_eq!(structured_event_level("translation_line_timeout"), "DBG");
    }

    #[test]
    fn classifies_subtitle_events() {
        assert_eq!(structured_event_level("subtitle_payload_published"), "DBG");
        assert_eq!(structured_event_level("subtitle_reset"), "DBG");
        assert_eq!(structured_event_level("overlay_update_published"), "DBG");
        assert_eq!(structured_event_level("overlay_stale_translation_suppressed"), "DBG");
        assert_eq!(structured_event_level("overlay_payload_mismatch"), "DBG");
        assert_eq!(structured_event_level("overlay_update_deduped"), "DBG");
        assert_eq!(structured_event_level("transcript_partial"), "DBG");
        assert_eq!(structured_event_level("presentation_built"), "DBG");
    }

    #[test]
    fn classifies_obs_events() {
        assert_eq!(structured_event_level("obs_caption_sent"), "DBG");
        assert_eq!(structured_event_level("obs_service_started"), "DBG");
        assert_eq!(structured_event_level("obs_connection_state_changed"), "DBG");
        assert_eq!(structured_event_level("obs_stream_output_inactive"), "DBG");
        assert_eq!(structured_event_level("obs_connection_lost"), "DBG");
        assert_eq!(structured_event_level("obs_caption_send_skipped"), "DBG");
        assert_eq!(structured_event_level("obs_caption_send_failed"), "ERR");
        assert_eq!(structured_event_level("obs_partial_throttled"), "DBG");
        assert_eq!(structured_event_level("obs_payload_routed"), "DBG");
    }

    #[test]
    fn compact_mode_keeps_inf_wrn_err_only() {
        assert!(should_write_runtime_event("translation_publish_accepted", false));
        assert!(should_write_runtime_event("translation_line_error", false));
        assert!(!should_write_runtime_event("translation_job_started", false));
        assert!(!should_write_runtime_event("subtitle_payload_published", false));
        assert!(!should_write_runtime_event("runtime_status_broadcast", false));
        assert!(!should_write_runtime_event("asr_ingest_partial_published", false));
        assert!(should_write_runtime_event("translation_job_started", true));
    }

    #[test]
    fn format_value_truncates_utf8_on_char_boundary() {
        let mut text = String::new();
        while text.len() < 210 {
            text.push('言');
        }
        let line = format_value("active_partial_text", &Value::String(text), 200);
        assert!(line.is_ascii() || std::str::from_utf8(line.as_bytes()).is_ok());
        assert!(line.contains('…'));
    }
}
