from __future__ import annotations

import unittest

from backend.core.runtime.partial_emit_coordinator import PartialEmitCoordinator
from backend.core.runtime.segment_state_controller import SegmentStateController


class PartialEmitCoordinatorTests(unittest.TestCase):
    def test_uses_effective_settings_and_segment_state(self) -> None:
        state = SegmentStateController()
        settings = {
            "partial_emit_mode": "word_growth",
            "partial_min_new_words": 1,
            "partial_min_delta_chars": 0,
            "partial_coalescing_ms": 0,
        }
        coord = PartialEmitCoordinator(state, lambda: settings)
        self.assertTrue(coord.should_emit_partial("s1", "hello"))
        coord.mark_partial_emitted("s1", "hello")
        self.assertFalse(coord.should_emit_partial("s1", "hello"))
        self.assertTrue(coord.should_emit_partial("s1", "hello world"))
        coord.mark_partial_emitted("s1", "hello world")

    def test_clear_partial_tracking_allows_repeat(self) -> None:
        state = SegmentStateController()
        coord = PartialEmitCoordinator(
            state,
            lambda: {
                "partial_emit_mode": "word_growth",
                "partial_min_new_words": 1,
                "partial_min_delta_chars": 0,
                "partial_coalescing_ms": 0,
            },
        )
        self.assertTrue(coord.should_emit_partial("s2", "a"))
        coord.mark_partial_emitted("s2", "a")
        coord.clear_partial_tracking_for_segment("s2")
        self.assertTrue(coord.should_emit_partial("s2", "a"))


class LocalAsrVadTuningTests(unittest.TestCase):
    def test_delta_enqueue_disabled_for_browser(self) -> None:
        from backend.core.runtime.local_asr_vad_tuning import local_asr_streaming_delta_enqueue_enabled

        self.assertFalse(
            local_asr_streaming_delta_enqueue_enabled(lambda: {"asr": {"realtime": {"streaming_decode": True}}}, is_browser_asr_mode=True)
        )

    def test_delta_enqueue_reads_streaming_decode(self) -> None:
        from backend.core.runtime.local_asr_vad_tuning import local_asr_streaming_delta_enqueue_enabled

        self.assertFalse(
            local_asr_streaming_delta_enqueue_enabled(
                lambda: {"asr": {"realtime": {"streaming_decode": "off"}}},
                is_browser_asr_mode=False,
            )
        )
        self.assertTrue(
            local_asr_streaming_delta_enqueue_enabled(
                lambda: {"asr": {"realtime": {"streaming_decode": True}}},
                is_browser_asr_mode=False,
            )
        )


if __name__ == "__main__":
    unittest.main()
