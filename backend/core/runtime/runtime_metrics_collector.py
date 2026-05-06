from __future__ import annotations

import time
from typing import Any, Literal

from backend.models import RuntimeMetrics


MetricKey = Literal["partial_updates_emitted", "finals_emitted", "suppressed_partial_updates"]
CounterMetricKey = Literal[
    "remote_audio_chunks_in",
    "remote_audio_bytes_in",
    "remote_audio_chunks_dropped",
    "vad_segments_partial",
    "vad_segments_final",
    "runtime_events_duplicate_suppressed",
    "runtime_status_broadcast_count",
    "runtime_status_duplicate_suppressed",
    "runtime_status_heartbeat_sent",
    "browser_worker_event_count",
    "browser_worker_event_coalesced",
    "overlay_stale_translation_suppressed",
    "overlay_payload_mismatch_count",
]


def record_metrics(metrics: RuntimeMetrics, **values: float | int | None) -> RuntimeMetrics:
    updates: dict[str, float | int] = {}
    for key, value in values.items():
        if value is None:
            continue
        if isinstance(value, int) and not isinstance(value, bool):
            updates[key] = int(value)
        else:
            updates[key] = round(float(value), 2)
    return metrics.model_copy(update=updates)


def increment_metric(metrics: RuntimeMetrics, key: MetricKey) -> RuntimeMetrics:
    current = getattr(metrics, key, 0) or 0
    return record_metrics(metrics, **{key: int(current) + 1})


def increment_counter_metric(metrics: RuntimeMetrics, key: CounterMetricKey, amount: int = 1) -> RuntimeMetrics:
    current = getattr(metrics, key, 0) or 0
    return record_metrics(metrics, **{key: int(current) + int(amount)})


def runtime_material_status_snapshot(payload: dict[str, Any]) -> tuple[Any, ...]:
    asr = payload.get("asr", {}) if isinstance(payload.get("asr"), dict) else {}
    asr_diagnostics = payload.get("asr_diagnostics", {}) if isinstance(payload.get("asr_diagnostics"), dict) else {}
    browser_worker = (
        asr_diagnostics.get("browser_worker", {})
        if isinstance(asr_diagnostics.get("browser_worker"), dict)
        else {}
    )
    return (
        payload.get("is_running"),
        payload.get("status"),
        payload.get("last_error"),
        payload.get("status_message"),
        asr.get("active_mode"),
        asr.get("effective_provider"),
        asr.get("provider"),
        asr.get("provider_phase"),
        asr.get("provider_message"),
        asr.get("provider_error_kind"),
        browser_worker.get("worker_connected"),
        browser_worker.get("recognition_state"),
        browser_worker.get("supervisor_state"),
        browser_worker.get("degraded_reason"),
        browser_worker.get("last_error"),
        browser_worker.get("generation_id"),
    )


def next_event_sequence(
    metrics: RuntimeMetrics,
    *,
    runtime_event_sequence: int,
    runtime_event_sequence_by_type: dict[str, int],
    event_type: str,
) -> tuple[RuntimeMetrics, int]:
    next_sequence = runtime_event_sequence + 1
    runtime_event_sequence_by_type[event_type] = next_sequence
    metrics = record_metrics(
        metrics,
        runtime_events_emitted=int(metrics.runtime_events_emitted or 0) + 1,
        runtime_events_last_sequence=next_sequence,
    )
    return metrics, next_sequence


def enrich_event_payload(
    metrics: RuntimeMetrics,
    *,
    payload: dict[str, Any],
    event_type: str,
    runtime_event_sequence: int,
    runtime_event_sequence_by_type: dict[str, int],
) -> tuple[RuntimeMetrics, int, dict[str, Any]]:
    metrics, next_sequence_value = next_event_sequence(
        metrics,
        runtime_event_sequence=runtime_event_sequence,
        runtime_event_sequence_by_type=runtime_event_sequence_by_type,
        event_type=event_type,
    )
    enriched = dict(payload)
    enriched.setdefault("event_type", event_type)
    enriched["created_at_ms"] = int(time.time() * 1000)
    enriched["event_sequence"] = next_sequence_value
    return metrics, next_sequence_value, enriched


def apply_translation_dispatcher_metrics(
    metrics: RuntimeMetrics,
    *,
    snapshot: dict[str, Any],
) -> RuntimeMetrics:
    updates: dict[str, float | int | None] = {}
    for key in (
        "translation_queue_depth",
        "translation_jobs_started",
        "translation_jobs_cancelled",
        "translation_stale_results_dropped",
        "translation_queue_latency_ms",
        "translation_provider_latency_ms",
    ):
        if key in snapshot:
            updates[key] = snapshot.get(key)
    if snapshot.get("translation_provider_latency_ms") is not None:
        updates["translation_ms"] = snapshot.get("translation_provider_latency_ms")
    return record_metrics(metrics, **updates)
