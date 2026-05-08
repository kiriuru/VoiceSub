from __future__ import annotations

import asyncio
import unittest

from backend.core.runtime.runtime_state_controller import RuntimeStateController
from backend.models import RuntimeMetrics, RuntimeState


class _RecordingWsManager:
    def __init__(self) -> None:
        self.messages: list[dict] = []

    async def broadcast(self, message: dict) -> None:
        self.messages.append(message)


class RuntimeEventSequenceMonotonicTests(unittest.TestCase):
    """
    Regression coverage for the dashboard "stuck on listening / texts not updating"
    bug: dashboard ws clients track the highest event_sequence per event type for
    staleness detection, so the backend must keep the runtime event_sequence
    monotonic across stop/start cycles. Resetting it back to 0 caused every event
    after a Stop/Start to be dropped client-side until the new session caught up
    to the previous session's high water mark.
    """

    def _make_controller(self) -> tuple[RuntimeStateController, dict[str, RuntimeMetrics], _RecordingWsManager]:
        metrics_holder = {"current": RuntimeMetrics()}

        def metrics_getter() -> RuntimeMetrics:
            return metrics_holder["current"]

        def metrics_setter(metrics: RuntimeMetrics) -> None:
            metrics_holder["current"] = metrics

        ws_manager = _RecordingWsManager()
        controller = RuntimeStateController(
            ws_manager,  # type: ignore[arg-type]
            metrics_getter=metrics_getter,
            metrics_setter=metrics_setter,
            increment_counter_metric=lambda _key, _amount: None,
            heartbeat_interval_ms=0,
        )
        return controller, metrics_holder, ws_manager

    def test_event_sequence_stays_monotonic_after_reset_broadcast_state(self) -> None:
        controller, _metrics, _ws = self._make_controller()
        first = controller.enrich("runtime_status", {"status": "listening"})
        second = controller.enrich("runtime_status", {"status": "transcribing"})
        controller.reset_broadcast_state()
        third = controller.enrich("runtime_status", {"status": "listening"})

        self.assertEqual(first["event_sequence"], 1)
        self.assertEqual(second["event_sequence"], 2)
        # The reset must NOT rewind event_sequence; otherwise long-lived ws
        # clients drop events after a runtime stop/start.
        self.assertEqual(third["event_sequence"], 3)
        self.assertGreater(third["event_sequence"], second["event_sequence"])

    def test_reset_broadcast_state_still_clears_status_signature(self) -> None:
        controller, _metrics, ws_manager = self._make_controller()

        async def scenario() -> None:
            state = RuntimeState(is_running=True, running=True, status="listening", phase="listening")
            await controller.broadcast_runtime(state)
            controller.reset_broadcast_state()
            # After reset, an unchanged signature must still produce a fresh broadcast
            # because the previous signature was cleared.
            await controller.broadcast_runtime(state)

        asyncio.run(scenario())
        self.assertEqual(len(ws_manager.messages), 2)
        first_seq = ws_manager.messages[0]["payload"]["event_sequence"]
        second_seq = ws_manager.messages[1]["payload"]["event_sequence"]
        self.assertGreater(second_seq, first_seq)

    def test_event_sequence_stays_monotonic_across_multiple_resets(self) -> None:
        controller, _metrics, _ws = self._make_controller()
        sequences: list[int] = []
        for cycle in range(3):
            for _ in range(4):
                enriched = controller.enrich("runtime_status", {"cycle": cycle})
                sequences.append(int(enriched["event_sequence"]))
            controller.reset_broadcast_state()
        self.assertEqual(sequences, sorted(sequences))
        self.assertEqual(len(sequences), len(set(sequences)))


if __name__ == "__main__":
    unittest.main()
