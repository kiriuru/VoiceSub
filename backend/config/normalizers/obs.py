from __future__ import annotations

from typing import Any

from backend.core.obs_caption_output import OBS_CC_OUTPUT_MODES


def normalize_obs_closed_captions_config(payload: Any, *, defaults: dict[str, Any]) -> dict[str, Any]:
    obs_closed_captions = payload if isinstance(payload, dict) else {}
    obs_connection = obs_closed_captions.get("connection", {})
    if not isinstance(obs_connection, dict):
        obs_connection = {}
    obs_debug_mirror = obs_closed_captions.get("debug_mirror", {})
    if not isinstance(obs_debug_mirror, dict):
        obs_debug_mirror = {}
    obs_timing = obs_closed_captions.get("timing", {})
    if not isinstance(obs_timing, dict):
        obs_timing = {}

    try:
        obs_port = int(obs_connection.get("port", defaults["connection"]["port"]) or defaults["connection"]["port"])
    except (TypeError, ValueError):
        obs_port = int(defaults["connection"]["port"])

    def clamp_obs_int(key: str, default: int) -> int:
        try:
            value = int(obs_timing.get(key, default) or default)
        except (TypeError, ValueError):
            value = default
        return max(0, value)

    output_mode = str(obs_closed_captions.get("output_mode", "disabled") or "disabled").strip().lower()
    if output_mode not in OBS_CC_OUTPUT_MODES:
        output_mode = "disabled"

    return {
        "enabled": bool(obs_closed_captions.get("enabled", False)),
        "output_mode": output_mode,
        "connection": {
            "host": str(obs_connection.get("host", defaults["connection"]["host"]) or defaults["connection"]["host"]).strip()
            or defaults["connection"]["host"],
            "port": max(1, min(65535, obs_port)),
            "password": str(obs_connection.get("password", "") or ""),
        },
        "debug_mirror": {
            "enabled": bool(obs_debug_mirror.get("enabled", False)),
            "input_name": str(obs_debug_mirror.get("input_name", defaults["debug_mirror"]["input_name"]) or defaults["debug_mirror"]["input_name"]).strip(),
            "send_partials": bool(obs_debug_mirror.get("send_partials", defaults["debug_mirror"]["send_partials"])),
        },
        "timing": {
            "send_partials": bool(obs_timing.get("send_partials", defaults["timing"]["send_partials"])),
            "partial_throttle_ms": clamp_obs_int("partial_throttle_ms", int(defaults["timing"]["partial_throttle_ms"])),
            "min_partial_delta_chars": clamp_obs_int("min_partial_delta_chars", int(defaults["timing"]["min_partial_delta_chars"])),
            "final_replace_delay_ms": clamp_obs_int("final_replace_delay_ms", int(defaults["timing"]["final_replace_delay_ms"])),
            "clear_after_ms": clamp_obs_int("clear_after_ms", int(defaults["timing"]["clear_after_ms"])),
            "avoid_duplicate_text": bool(obs_timing.get("avoid_duplicate_text", defaults["timing"]["avoid_duplicate_text"])),
        },
    }
