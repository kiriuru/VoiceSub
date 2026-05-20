"""Local capture: VAD configuration and streaming-delta queue flag (orchestrator glue)."""

from __future__ import annotations

from typing import Any, Callable

from backend.core.vad import VadEngine


def apply_vad_tuning_from_settings(
    vad: VadEngine,
    *,
    realtime_settings: dict[str, Any],
    lifecycle_settings: dict[str, Any],
) -> None:
    vad.configure(
        mode=int(realtime_settings["vad_mode"]),
        silence_hold_ms=realtime_settings["silence_hold_ms"],
        finalization_hold_ms=int(lifecycle_settings["pause_to_finalize_ms"]),
        min_speech_ms=realtime_settings["min_speech_ms"],
        partial_emit_interval_ms=realtime_settings["partial_emit_interval_ms"],
        max_segment_ms=int(lifecycle_settings["hard_max_phrase_ms"]),
        energy_gate_enabled=bool(realtime_settings["energy_gate_enabled"]),
        min_rms_for_recognition=float(realtime_settings["min_rms_for_recognition"]),
        min_voiced_ratio=float(realtime_settings["min_voiced_ratio"]),
        first_partial_min_speech_ms=int(realtime_settings["first_partial_min_speech_ms"]),
        speech_attack_frames=int(realtime_settings.get("vad_speech_attack_frames", 2) or 2),
        speech_preroll_frames=int(realtime_settings.get("vad_speech_preroll_frames", 5) or 5),
    )


def local_asr_streaming_delta_enqueue_enabled(
    config_getter: Callable[[], dict],
    *,
    is_browser_asr_mode: bool,
) -> bool:
    if is_browser_asr_mode:
        return False
    config = config_getter()
    asr_config = config.get("asr", {}) if isinstance(config, dict) else {}
    realtime = asr_config.get("realtime", {}) if isinstance(asr_config, dict) else {}
    if not isinstance(realtime, dict):
        return False
    value = realtime.get("streaming_decode", True)
    if isinstance(value, str):
        return value.strip().lower() not in {"0", "false", "no", "off"}
    return bool(value)
