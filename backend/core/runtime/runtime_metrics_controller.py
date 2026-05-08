from __future__ import annotations

from typing import Any, Literal

from backend.models import RuntimeMetrics
from backend.core.runtime.runtime_metrics_collector import (
    apply_translation_dispatcher_metrics,
    increment_counter_metric,
    increment_metric,
    record_metrics,
)


class RuntimeMetricsController:
    """
    Owns RuntimeMetrics state for RuntimeOrchestrator.

    This keeps metric mutation in one place while preserving the existing
    RuntimeMetrics model and runtime status payload shape.
    """

    name = "runtime_metrics"

    def __init__(self) -> None:
        self._metrics = RuntimeMetrics()

    @property
    def metrics(self) -> RuntimeMetrics:
        return self._metrics

    def set_metrics(self, metrics: RuntimeMetrics) -> None:
        self._metrics = metrics if isinstance(metrics, RuntimeMetrics) else RuntimeMetrics()

    def reset(self) -> None:
        self._metrics = RuntimeMetrics()

    def record(self, **values: float | int | None) -> None:
        self._metrics = record_metrics(self._metrics, **values)

    def increment_metric(
        self,
        key: Literal[
            "partial_updates_emitted",
            "finals_emitted",
            "suppressed_partial_updates",
        ],
    ) -> None:
        self._metrics = increment_metric(self._metrics, key)

    def increment_counter_metric(self, key: str, amount: int = 1) -> None:
        self._metrics = increment_counter_metric(self._metrics, key, amount)

    def apply_translation_dispatcher_metrics(self, snapshot: dict) -> None:
        if not isinstance(snapshot, dict):
            return
        self._metrics = apply_translation_dispatcher_metrics(self._metrics, snapshot=snapshot)

    def diagnostics(self) -> dict[str, Any]:
        return self._metrics.model_dump()
