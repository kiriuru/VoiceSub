from __future__ import annotations

import time
from typing import Any


class SegmentStateController:
    """
    Owns segment/partial tracking state used by RuntimeOrchestrator.
    """

    name = "segment_state"

    def __init__(self) -> None:
        self._sequence: int = 0
        self._segment_counter: int = 0
        self._active_segment_id: str | None = None
        self._active_segment_revision: int = 0
        self._last_partial_text_by_segment: dict[str, str] = {}
        self._last_partial_emit_monotonic_by_segment: dict[str, float] = {}

    @property
    def sequence(self) -> int:
        return self._sequence

    @property
    def segment_counter(self) -> int:
        return self._segment_counter

    @property
    def active_segment_id(self) -> str | None:
        return self._active_segment_id

    @property
    def active_segment_revision(self) -> int:
        return self._active_segment_revision

    def reset_sequence(self) -> None:
        self._sequence = 0

    def next_sequence(self) -> int:
        self._sequence += 1
        return self._sequence

    def reset_segment_counter(self) -> None:
        self._segment_counter = 0

    def next_segment_id(self, *, prefix: str = "segment") -> str:
        self._segment_counter += 1
        return f"{prefix}-{self._segment_counter}"

    def set_active_segment(self, segment_id: str | None, *, revision: int | None = None) -> None:
        self._active_segment_id = str(segment_id) if segment_id is not None else None
        if revision is not None:
            self._active_segment_revision = int(revision)

    def clear_active_segment(self) -> None:
        self._active_segment_id = None
        self._active_segment_revision = 0

    def bump_active_segment_revision(self) -> int:
        self._active_segment_revision += 1
        return self._active_segment_revision

    def clear_all_partial_tracking(self) -> None:
        self._last_partial_text_by_segment.clear()
        self._last_partial_emit_monotonic_by_segment.clear()

    def clear_partial_tracking_for_segment(self, segment_id: str | None) -> None:
        if not segment_id:
            return
        self._last_partial_text_by_segment.pop(segment_id, None)
        self._last_partial_emit_monotonic_by_segment.pop(segment_id, None)

    def get_last_partial_text(self, segment_id: str | None) -> str:
        if not segment_id:
            return ""
        return self._last_partial_text_by_segment.get(segment_id, "")

    def set_last_partial_text(self, segment_id: str | None, text: str) -> None:
        if not segment_id:
            return
        self._last_partial_text_by_segment[segment_id] = str(text or "")

    def get_last_partial_emit_monotonic(self, segment_id: str | None) -> float | None:
        if not segment_id:
            return None
        return self._last_partial_emit_monotonic_by_segment.get(segment_id)

    def set_last_partial_emit_monotonic(self, segment_id: str | None, value: float) -> None:
        if not segment_id:
            return
        self._last_partial_emit_monotonic_by_segment[segment_id] = float(value)

    def mark_partial_emitted(self, segment_id: str | None, text: str) -> None:
        normalized = " ".join(str(text or "").split())
        if not segment_id:
            return
        self._last_partial_text_by_segment[segment_id] = normalized
        self._last_partial_emit_monotonic_by_segment[segment_id] = time.perf_counter()

    def assign_segment_tracking(
        self,
        *,
        preferred_segment_id: str | None = None,
    ) -> tuple[str, int, bool, str | None]:
        """
        Returns (segment_id, revision, started_now, previous_segment_id_to_clear_tracking).
        """
        started_now = False
        previous_to_clear: str | None = None
        normalized_preferred = str(preferred_segment_id or "").strip() or None

        if normalized_preferred and normalized_preferred != self._active_segment_id:
            previous_to_clear = self._active_segment_id
            self._active_segment_id = normalized_preferred
            self._active_segment_revision = 0
            started_now = True
        elif self._active_segment_id is None:
            self._active_segment_id = self.next_segment_id(prefix="segment")
            self._active_segment_revision = 0
            started_now = True

        revision = self.bump_active_segment_revision()
        return str(self._active_segment_id), revision, started_now, previous_to_clear

    def cleanup_on_browser_worker_disconnect(self) -> None:
        segment_id = self._active_segment_id
        self.clear_active_segment()
        self.clear_partial_tracking_for_segment(segment_id)

    def diagnostics(self) -> dict[str, Any]:
        return {
            "sequence": self._sequence,
            "segment_counter": self._segment_counter,
            "active_segment_id": self._active_segment_id,
            "active_segment_revision": self._active_segment_revision,
            "tracked_partial_segments": len(self._last_partial_text_by_segment),
            "tracked_partial_emit_segments": len(self._last_partial_emit_monotonic_by_segment),
        }

