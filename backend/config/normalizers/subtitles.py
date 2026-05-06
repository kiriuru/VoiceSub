from __future__ import annotations

from typing import Any


def normalize_display_order(*, display_order: list[Any], target_languages: list[str]) -> list[str]:
    normalized_order: list[str] = []
    for item in display_order:
        value = str(item).lower()
        if value == "source" or value in target_languages:
            if value not in normalized_order:
                normalized_order.append(value)
    if "source" not in normalized_order:
        normalized_order.append("source")
    for target_lang in target_languages:
        if target_lang not in normalized_order:
            normalized_order.append(target_lang)
    return normalized_order


def normalize_subtitle_output_config(payload: Any, *, target_languages: list[str]) -> dict[str, Any]:
    subtitle_output = payload if isinstance(payload, dict) else {}
    display_order = subtitle_output.get("display_order", ["source", *target_languages])
    if not isinstance(display_order, list):
        display_order = ["source", *target_languages]
    try:
        max_translation_languages = int(subtitle_output.get("max_translation_languages", 2) or 0)
    except (TypeError, ValueError):
        max_translation_languages = 2
    return {
        "show_source": bool(subtitle_output.get("show_source", True)),
        "show_translations": bool(subtitle_output.get("show_translations", True)),
        "max_translation_languages": max(0, min(5, max_translation_languages)),
        "display_order": normalize_display_order(
            display_order=display_order,
            target_languages=target_languages,
        ),
    }


def normalize_subtitle_lifecycle_config(
    payload: Any,
    *,
    defaults: dict[str, Any],
    fallback_realtime: dict[str, int] | None = None,
    fallback_realtime_defaults: dict[str, Any] | None = None,
) -> dict[str, Any]:
    current = payload if isinstance(payload, dict) else {}
    realtime = fallback_realtime if isinstance(fallback_realtime, dict) else (fallback_realtime_defaults or {})

    def clamp_int_value(raw: Any, *, default: int, minimum: int, maximum: int) -> int:
        try:
            value = int(raw)
        except (TypeError, ValueError):
            value = int(default)
        return max(minimum, min(maximum, value))

    pause_default = int(realtime.get("finalization_hold_ms", defaults["pause_to_finalize_ms"]))
    hard_max_default = int(realtime.get("max_segment_ms", defaults["hard_max_phrase_ms"]))
    completed_ttl_default = clamp_int_value(
        current.get("completed_block_ttl_ms", defaults["completed_block_ttl_ms"]),
        default=defaults["completed_block_ttl_ms"],
        minimum=500,
        maximum=20000,
    )
    source_ttl = clamp_int_value(
        current.get("completed_source_ttl_ms", completed_ttl_default),
        default=completed_ttl_default,
        minimum=500,
        maximum=20000,
    )
    translation_ttl = clamp_int_value(
        current.get("completed_translation_ttl_ms", completed_ttl_default),
        default=completed_ttl_default,
        minimum=500,
        maximum=20000,
    )

    return {
        "completed_block_ttl_ms": max(source_ttl, translation_ttl),
        "completed_source_ttl_ms": source_ttl,
        "completed_translation_ttl_ms": translation_ttl,
        "pause_to_finalize_ms": clamp_int_value(
            current.get("pause_to_finalize_ms", pause_default),
            default=pause_default,
            minimum=120,
            maximum=5000,
        ),
        "allow_early_replace_on_next_final": bool(
            current.get("allow_early_replace_on_next_final", defaults["allow_early_replace_on_next_final"])
        ),
        "sync_source_and_translation_expiry": bool(
            current.get("sync_source_and_translation_expiry", defaults["sync_source_and_translation_expiry"])
        ),
        "keep_completed_translation_during_active_partial": bool(
            current.get(
                "keep_completed_translation_during_active_partial",
                defaults["keep_completed_translation_during_active_partial"],
            )
        ),
        "hard_max_phrase_ms": clamp_int_value(
            current.get("hard_max_phrase_ms", hard_max_default),
            default=hard_max_default,
            minimum=1000,
            maximum=30000,
        ),
    }
