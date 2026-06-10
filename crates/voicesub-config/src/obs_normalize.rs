use serde_json::{json, Map, Value};

pub const OBS_CC_OUTPUT_MODES: &[&str] = &[
    "disabled",
    "source_live",
    "source_final_only",
    "translation_1",
    "translation_2",
    "translation_3",
    "translation_4",
    "translation_5",
    "first_visible_line",
];

fn obs_defaults() -> Value {
    json!({
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
    })
}

fn clamp_obs_int(section: &Map<String, Value>, key: &str, default: i64) -> i64 {
    let value = section
        .get(key)
        .and_then(|value| value.as_i64().or_else(|| value.as_u64().map(|n| n as i64)))
        .unwrap_or(default);
    value.max(0)
}

pub fn normalize_obs_closed_captions(root: &mut Map<String, Value>) {
    let defaults = obs_defaults();
    let default_connection = defaults["connection"].as_object().cloned().unwrap_or_default();
    let default_debug = defaults["debug_mirror"].as_object().cloned().unwrap_or_default();
    let default_timing = defaults["timing"].as_object().cloned().unwrap_or_default();

    let section_value = root
        .entry("obs_closed_captions".to_string())
        .or_insert_with(|| json!({}));
    let section = section_value
        .as_object_mut()
        .expect("obs_closed_captions object");

    let raw_mode = section
        .get("output_mode")
        .and_then(|value| value.as_str())
        .unwrap_or("disabled")
        .trim()
        .to_ascii_lowercase();
    let output_mode = if OBS_CC_OUTPUT_MODES.contains(&raw_mode.as_str()) {
        raw_mode
    } else {
        "disabled".to_string()
    };

    let enabled = section
        .get("enabled")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    let connection_host = section
        .get("connection")
        .and_then(|value| value.get("host"))
        .and_then(|value| value.as_str())
        .unwrap_or(default_connection["host"].as_str().unwrap_or("127.0.0.1"))
        .trim()
        .to_string();
    let connection_port = section
        .get("connection")
        .and_then(|value| value.get("port"))
        .and_then(|value| value.as_i64().or_else(|| value.as_u64().map(|n| n as i64)))
        .unwrap_or(4455)
        .clamp(1, 65535);
    let connection_password = section
        .get("connection")
        .and_then(|value| value.get("password"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();

    let debug_enabled = section
        .get("debug_mirror")
        .and_then(|value| value.get("enabled"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let debug_input_name = section
        .get("debug_mirror")
        .and_then(|value| value.get("input_name"))
        .and_then(|value| value.as_str())
        .unwrap_or(default_debug["input_name"].as_str().unwrap_or("CC_DEBUG"))
        .trim()
        .to_string();
    let debug_send_partials = section
        .get("debug_mirror")
        .and_then(|value| value.get("send_partials"))
        .and_then(|value| value.as_bool())
        .unwrap_or(true);

    let timing_send_partials = section
        .get("timing")
        .and_then(|value| value.get("send_partials"))
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    let timing_map = section
        .get("timing")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default();
    let partial_throttle_ms = clamp_obs_int(
        &timing_map,
        "partial_throttle_ms",
        default_timing["partial_throttle_ms"].as_i64().unwrap_or(140),
    );
    let min_partial_delta_chars = clamp_obs_int(
        &timing_map,
        "min_partial_delta_chars",
        default_timing["min_partial_delta_chars"].as_i64().unwrap_or(1),
    );
    let final_replace_delay_ms = clamp_obs_int(
        &timing_map,
        "final_replace_delay_ms",
        default_timing["final_replace_delay_ms"].as_i64().unwrap_or(0),
    );
    let clear_after_ms = clamp_obs_int(
        &timing_map,
        "clear_after_ms",
        default_timing["clear_after_ms"].as_i64().unwrap_or(2500),
    );
    let avoid_duplicate_text = section
        .get("timing")
        .and_then(|value| value.get("avoid_duplicate_text"))
        .and_then(|value| value.as_bool())
        .unwrap_or(true);

    section.insert("enabled".into(), json!(enabled));
    section.insert("output_mode".into(), json!(output_mode));
    section.insert(
        "connection".into(),
        json!({
            "host": if connection_host.is_empty() { "127.0.0.1" } else { connection_host.as_str() },
            "port": connection_port,
            "password": connection_password,
        }),
    );
    section.insert(
        "debug_mirror".into(),
        json!({
            "enabled": debug_enabled,
            "input_name": debug_input_name,
            "send_partials": debug_send_partials,
        }),
    );
    section.insert(
        "timing".into(),
        json!({
            "send_partials": timing_send_partials,
            "partial_throttle_ms": partial_throttle_ms,
            "min_partial_delta_chars": min_partial_delta_chars,
            "final_replace_delay_ms": final_replace_delay_ms,
            "clear_after_ms": clear_after_ms,
            "avoid_duplicate_text": avoid_duplicate_text,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_invalid_output_mode_to_disabled() {
        let mut root = Map::new();
        root.insert(
            "obs_closed_captions".into(),
            json!({ "output_mode": "bogus", "timing": {} }),
        );
        normalize_obs_closed_captions(&mut root);
        assert_eq!(root["obs_closed_captions"]["output_mode"], "disabled");
        assert_eq!(root["obs_closed_captions"]["timing"]["partial_throttle_ms"], 140);
    }
}
