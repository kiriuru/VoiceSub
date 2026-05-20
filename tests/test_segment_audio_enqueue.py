from __future__ import annotations

import unittest

from backend.core.runtime.segment_audio_enqueue import (
    clear_segment_audio_enqueue_state,
    slice_segment_audio_delta,
)


class SegmentAudioEnqueueTests(unittest.TestCase):
    def test_slice_returns_only_new_suffix(self) -> None:
        tracker: dict[str, int] = {}
        cumulative = b"abcdefghij"

        delta1, skip1 = slice_segment_audio_delta(
            segment_audio=cumulative[:4],
            segment_id="seg-1",
            started_now=True,
            queued_byte_len_by_segment=tracker,
        )
        self.assertFalse(skip1)
        self.assertEqual(delta1, b"abcd")

        delta2, skip2 = slice_segment_audio_delta(
            segment_audio=cumulative,
            segment_id="seg-1",
            started_now=False,
            queued_byte_len_by_segment=tracker,
        )
        self.assertFalse(skip2)
        self.assertEqual(delta2, b"efghij")

    def test_skip_when_no_new_audio(self) -> None:
        tracker: dict[str, int] = {"seg-1": 4}
        delta, skip = slice_segment_audio_delta(
            segment_audio=b"abcd",
            segment_id="seg-1",
            started_now=False,
            queued_byte_len_by_segment=tracker,
        )
        self.assertTrue(skip)
        self.assertEqual(delta, b"")

    def test_clear_segment_state(self) -> None:
        tracker = {"seg-1": 10, "seg-2": 3}
        clear_segment_audio_enqueue_state(tracker, segment_id="seg-1")
        self.assertNotIn("seg-1", tracker)
        self.assertIn("seg-2", tracker)
        clear_segment_audio_enqueue_state(tracker)
        self.assertEqual(tracker, {})
