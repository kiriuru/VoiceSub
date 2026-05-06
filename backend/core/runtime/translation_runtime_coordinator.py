from __future__ import annotations

from typing import Any, Callable

from backend.models import TranslationDiagnostics


def summarize_translation_diagnostics(
    *,
    config_getter: Callable[[], dict],
    translation_engine: Any,
    translation_dispatcher_snapshot: dict[str, Any],
) -> TranslationDiagnostics:
    try:
        config = config_getter()
        translation_config = config.get("translation", {}) if isinstance(config, dict) else {}
        diagnostics = translation_engine.summarize_readiness(
            translation_config if isinstance(translation_config, dict) else {}
        )
        snapshot = translation_dispatcher_snapshot if isinstance(translation_dispatcher_snapshot, dict) else {}
        runtime_reason = snapshot.get("translation_last_runtime_reason")
        return diagnostics.model_copy(
            update={
                "queue_depth": int(snapshot.get("translation_queue_depth", 0) or 0),
                "jobs_started": int(snapshot.get("translation_jobs_started", 0) or 0),
                "jobs_cancelled": int(snapshot.get("translation_jobs_cancelled", 0) or 0),
                "stale_results_dropped": int(snapshot.get("translation_stale_results_dropped", 0) or 0),
                "last_queue_latency_ms": snapshot.get("translation_queue_latency_ms"),
                "last_provider_latency_ms": snapshot.get("translation_provider_latency_ms"),
                "last_runtime_reason": str(runtime_reason).strip() or None if runtime_reason is not None else None,
            }
        )
    except Exception as exc:
        return TranslationDiagnostics(
            enabled=False,
            status="error",
            summary="Translation diagnostics unavailable.",
            reason=str(exc),
            degraded=True,
        )
