"""Partial transcript emission gate (RealtimeTranscriptEmitPolicy + segment state)."""

from __future__ import annotations

import time
from typing import Any, Callable

from backend.core.runtime.realtime_transcript_emit_policy import should_emit_partial as should_emit_realtime_partial
from backend.core.runtime.segment_state_controller import SegmentStateController


class PartialEmitCoordinator:
    """Keeps partial emit decisions next to segment partial tracking (thin orchestrator slice)."""

    def __init__(
        self,
        segment_state: SegmentStateController,
        effective_realtime_settings_getter: Callable[[], dict[str, Any]],
    ) -> None:
        self._segment_state = segment_state
        self._effective_realtime_settings_getter = effective_realtime_settings_getter

    def should_emit_partial(self, segment_id: str, text: str) -> bool:
        settings = self._effective_realtime_settings_getter()
        emit_mode = str(settings.get("partial_emit_mode", "word_growth") or "word_growth")
        return should_emit_realtime_partial(
            new_text=text,
            previous_text=self._segment_state.get_last_partial_text(segment_id),
            mode=emit_mode,
            min_new_words=int(settings.get("partial_min_new_words", 1) or 1),
            min_delta_chars=int(settings.get("partial_min_delta_chars", 0) or 0),
            coalescing_ms=int(settings.get("partial_coalescing_ms", 0) or 0),
            previous_emit_monotonic=self._segment_state.get_last_partial_emit_monotonic(segment_id),
            now_monotonic=time.perf_counter(),
        )

    def mark_partial_emitted(self, segment_id: str, text: str) -> None:
        self._segment_state.mark_partial_emitted(segment_id, text)

    def clear_partial_tracking_for_segment(self, segment_id: str | None) -> None:
        self._segment_state.clear_partial_tracking_for_segment(segment_id)
