from __future__ import annotations

from typing import Any, Callable

from backend.core.asr_engine import AsrEngine
from backend.core.runtime.local_asr_constants import LEGACY_VAD_SETTINGS


def resolve_realtime_settings(
    *,
    config_getter: Callable[[], dict],
    asr_engine: AsrEngine,
    legacy_settings: dict[str, int | float | bool] | None = None,
) -> dict[str, int | float | bool]:
    base = dict(legacy_settings or LEGACY_VAD_SETTINGS)
    config = config_getter()
    asr_config = config.get("asr", {}) if isinstance(config, dict) else {}
    if not isinstance(asr_config, dict):
        asr_config = {}

    status = asr_engine.status()
    if status.provider != "official_eu_parakeet_low_latency":
        return base

    effective = dict(base)
    realtime_settings = asr_config.get("realtime", {})
    latency_preset = None
    if isinstance(realtime_settings, dict):
        latency_preset = str(realtime_settings.get("latency_preset", "") or "").strip().lower() or None
    if latency_preset in {"ultra_low_latency", "balanced", "quality"}:
        presets: dict[str, dict[str, int | bool]] = {
            "ultra_low_latency": {
                "first_partial_min_speech_ms": 120,
                "partial_emit_interval_ms": 240,
                "silence_hold_ms": 120,
                "finalization_hold_ms": 220,
                "partial_emit_mode": "word_growth",
                "partial_min_new_words": 1,
                "partial_min_delta_chars": 0,
                "partial_coalescing_ms": 0,
                "streaming_decode": True,
            },
            "balanced": {
                "first_partial_min_speech_ms": 180,
                "partial_emit_interval_ms": 280,
                "silence_hold_ms": 180,
                "finalization_hold_ms": 350,
                "partial_emit_mode": "word_growth",
                "partial_min_new_words": 1,
                "partial_min_delta_chars": 0,
                "partial_coalescing_ms": 0,
                "streaming_decode": True,
            },
            "quality": {
                "first_partial_min_speech_ms": 260,
                "partial_emit_interval_ms": 650,
                "silence_hold_ms": 260,
                "finalization_hold_ms": 520,
                "partial_emit_mode": "word_growth",
                "partial_min_new_words": 1,
                "partial_min_delta_chars": 1,
                "partial_coalescing_ms": 80,
                "streaming_decode": True,
            },
        }
        effective.update(presets.get(latency_preset, {}))
    if isinstance(realtime_settings, dict):
        for key in effective:
            value = realtime_settings.get(key)
            if isinstance(value, (int, float)):
                effective[key] = int(value)
    for key in ("vad_mode", "energy_gate_enabled", "min_rms_for_recognition", "min_voiced_ratio", "first_partial_min_speech_ms"):
        value = realtime_settings.get(key) if isinstance(realtime_settings, dict) else None
        if key in {"energy_gate_enabled"}:
            effective[key] = bool(value) if value is not None else effective[key]
        elif key in {"min_rms_for_recognition", "min_voiced_ratio"}:
            if isinstance(value, (int, float)):
                effective[key] = float(value)
        elif isinstance(value, (int, float)):
            effective[key] = int(value)
    return effective


def resolve_subtitle_lifecycle_settings(
    *,
    config_getter: Callable[[], dict],
    legacy_settings: dict[str, int | float | bool] | None = None,
) -> dict[str, int | bool]:
    base = dict(legacy_settings or LEGACY_VAD_SETTINGS)
    config = config_getter()
    lifecycle = config.get("subtitle_lifecycle", {}) if isinstance(config, dict) else {}
    if not isinstance(lifecycle, dict):
        lifecycle = {}
    completed_ttl_ms = max(500, int(lifecycle.get("completed_block_ttl_ms", 4500) or 4500))
    source_ttl_ms = max(500, int(lifecycle.get("completed_source_ttl_ms", completed_ttl_ms) or completed_ttl_ms))
    translation_ttl_ms = max(500, int(lifecycle.get("completed_translation_ttl_ms", completed_ttl_ms) or completed_ttl_ms))
    return {
        "completed_block_ttl_ms": max(source_ttl_ms, translation_ttl_ms),
        "completed_source_ttl_ms": source_ttl_ms,
        "completed_translation_ttl_ms": translation_ttl_ms,
        "pause_to_finalize_ms": max(
            120,
            int(lifecycle.get("pause_to_finalize_ms", base["finalization_hold_ms"]) or base["finalization_hold_ms"]),
        ),
        "allow_early_replace_on_next_final": bool(lifecycle.get("allow_early_replace_on_next_final", True)),
        "sync_source_and_translation_expiry": bool(lifecycle.get("sync_source_and_translation_expiry", True)),
        "keep_completed_translation_during_active_partial": bool(
            lifecycle.get("keep_completed_translation_during_active_partial", True)
        ),
        "hard_max_phrase_ms": max(
            1000,
            int(lifecycle.get("hard_max_phrase_ms", base["max_segment_ms"]) or base["max_segment_ms"]),
        ),
    }
