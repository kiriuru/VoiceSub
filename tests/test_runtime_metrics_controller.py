from __future__ import annotations

import unittest

from backend.core.runtime.runtime_metrics_controller import RuntimeMetricsController
from backend.models import RuntimeMetrics


class RuntimeMetricsControllerTests(unittest.TestCase):
    def test_initial_metrics(self) -> None:
        ctl = RuntimeMetricsController()
        self.assertIsInstance(ctl.metrics, RuntimeMetrics)

    def test_reset(self) -> None:
        ctl = RuntimeMetricsController()
        ctl.record(vad_ms=12.5)
        ctl.increment_metric("finals_emitted")
        ctl.reset()
        self.assertEqual(ctl.metrics.vad_ms, None)
        self.assertEqual(ctl.metrics.finals_emitted, 0)

    def test_record_latency_values(self) -> None:
        ctl = RuntimeMetricsController()
        ctl.record(
            vad_ms=10.0,
            asr_partial_ms=20.0,
            asr_final_ms=30.0,
            translation_ms=40.0,
            total_ms=100.0,
        )
        self.assertEqual(ctl.metrics.vad_ms, 10.0)
        self.assertEqual(ctl.metrics.asr_partial_ms, 20.0)
        self.assertEqual(ctl.metrics.asr_final_ms, 30.0)
        self.assertEqual(ctl.metrics.translation_ms, 40.0)
        self.assertEqual(ctl.metrics.total_ms, 100.0)

    def test_increment_standard_metric(self) -> None:
        ctl = RuntimeMetricsController()
        ctl.increment_metric("partial_updates_emitted")
        ctl.increment_metric("finals_emitted")
        ctl.increment_metric("suppressed_partial_updates")
        self.assertEqual(ctl.metrics.partial_updates_emitted, 1)
        self.assertEqual(ctl.metrics.finals_emitted, 1)
        self.assertEqual(ctl.metrics.suppressed_partial_updates, 1)

    def test_increment_counter_metric(self) -> None:
        ctl = RuntimeMetricsController()
        ctl.increment_counter_metric("runtime_status_broadcast_count", 2)
        ctl.increment_counter_metric("browser_worker_event_count", 1)
        self.assertEqual(ctl.metrics.runtime_status_broadcast_count, 2)
        self.assertEqual(ctl.metrics.browser_worker_event_count, 1)

    def test_apply_translation_dispatcher_metrics(self) -> None:
        ctl = RuntimeMetricsController()
        ctl.apply_translation_dispatcher_metrics(
            {
                "translation_queue_depth": 3,
                "translation_jobs_started": 10,
                "translation_jobs_cancelled": 1,
                "translation_stale_results_dropped": 2,
                "translation_queue_latency_ms": 5.5,
                "translation_provider_latency_ms": 99.0,
            }
        )
        self.assertEqual(ctl.metrics.translation_queue_depth, 3)
        self.assertEqual(ctl.metrics.translation_jobs_started, 10)
        self.assertEqual(ctl.metrics.translation_jobs_cancelled, 1)
        self.assertEqual(ctl.metrics.translation_stale_results_dropped, 2)
        self.assertEqual(ctl.metrics.translation_queue_latency_ms, 5.5)
        self.assertEqual(ctl.metrics.translation_provider_latency_ms, 99.0)
        self.assertEqual(ctl.metrics.translation_ms, 99.0)

    def test_set_metrics(self) -> None:
        ctl = RuntimeMetricsController()
        other = RuntimeMetrics(vad_ms=1.0)
        ctl.set_metrics(other)
        self.assertEqual(ctl.metrics.vad_ms, 1.0)

    def test_apply_non_dict_is_noop(self) -> None:
        ctl = RuntimeMetricsController()
        ctl.record(vad_ms=7.0)
        ctl.apply_translation_dispatcher_metrics("not-a-dict")  # type: ignore[arg-type]
        self.assertEqual(ctl.metrics.vad_ms, 7.0)


if __name__ == "__main__":
    unittest.main()
