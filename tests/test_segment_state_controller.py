from __future__ import annotations

import unittest

from backend.core.runtime.segment_state_controller import SegmentStateController


class SegmentStateControllerTests(unittest.TestCase):
    def test_default_state(self) -> None:
        c = SegmentStateController()
        self.assertEqual(c.sequence, 0)
        self.assertEqual(c.segment_counter, 0)
        self.assertIsNone(c.active_segment_id)
        self.assertEqual(c.active_segment_revision, 0)
        d = c.diagnostics()
        self.assertEqual(d["tracked_partial_segments"], 0)
        self.assertEqual(d["tracked_partial_emit_segments"], 0)

    def test_sequence_next_and_reset(self) -> None:
        c = SegmentStateController()
        self.assertEqual(c.next_sequence(), 1)
        self.assertEqual(c.next_sequence(), 2)
        self.assertEqual(c.sequence, 2)
        c.reset_sequence()
        self.assertEqual(c.sequence, 0)

    def test_segment_counter_and_id_format(self) -> None:
        c = SegmentStateController()
        self.assertEqual(c.next_segment_id(prefix="segment"), "segment-1")
        self.assertEqual(c.next_segment_id(prefix="segment"), "segment-2")
        self.assertEqual(c.segment_counter, 2)
        c.reset_segment_counter()
        self.assertEqual(c.segment_counter, 0)
        self.assertEqual(c.next_segment_id(prefix="segment"), "segment-1")

    def test_active_segment_and_revision(self) -> None:
        c = SegmentStateController()
        c.set_active_segment("segment-9", revision=0)
        self.assertEqual(c.active_segment_id, "segment-9")
        self.assertEqual(c.active_segment_revision, 0)
        self.assertEqual(c.bump_active_segment_revision(), 1)
        self.assertEqual(c.active_segment_revision, 1)
        c.clear_active_segment()
        self.assertIsNone(c.active_segment_id)
        self.assertEqual(c.active_segment_revision, 0)

    def test_partial_tracking_set_get_and_clear(self) -> None:
        c = SegmentStateController()
        self.assertEqual(c.get_last_partial_text("segment-1"), "")
        self.assertIsNone(c.get_last_partial_emit_monotonic("segment-1"))
        c.set_last_partial_text("segment-1", "hi")
        c.set_last_partial_emit_monotonic("segment-1", 12.5)
        self.assertEqual(c.get_last_partial_text("segment-1"), "hi")
        self.assertEqual(c.get_last_partial_emit_monotonic("segment-1"), 12.5)
        c.clear_partial_tracking_for_segment("segment-1")
        self.assertEqual(c.get_last_partial_text("segment-1"), "")
        self.assertIsNone(c.get_last_partial_emit_monotonic("segment-1"))

        c.set_last_partial_text("segment-1", "x")
        c.set_last_partial_text("segment-2", "y")
        c.clear_all_partial_tracking()
        self.assertEqual(c.get_last_partial_text("segment-1"), "")
        self.assertEqual(c.get_last_partial_text("segment-2"), "")

    def test_cleanup_on_browser_worker_disconnect(self) -> None:
        c = SegmentStateController()
        c.set_active_segment("segment-1", revision=3)
        c.set_last_partial_text("segment-1", "hello")
        c.cleanup_on_browser_worker_disconnect()
        self.assertIsNone(c.active_segment_id)
        self.assertEqual(c.active_segment_revision, 0)
        self.assertEqual(c.get_last_partial_text("segment-1"), "")

    def test_assign_segment_tracking_behavior(self) -> None:
        c = SegmentStateController()
        seg_id, rev, started_now, prev = c.assign_segment_tracking()
        self.assertTrue(started_now)
        self.assertIsNone(prev)
        self.assertEqual(seg_id, "segment-1")
        self.assertEqual(rev, 1)

        seg_id2, rev2, started_now2, prev2 = c.assign_segment_tracking()
        self.assertFalse(started_now2)
        self.assertIsNone(prev2)
        self.assertEqual(seg_id2, "segment-1")
        self.assertEqual(rev2, 2)

        seg_id3, rev3, started_now3, prev3 = c.assign_segment_tracking(preferred_segment_id="client-abc")
        self.assertTrue(started_now3)
        self.assertEqual(prev3, "segment-1")
        self.assertEqual(seg_id3, "client-abc")
        self.assertEqual(rev3, 1)

    def test_diagnostics_keys(self) -> None:
        c = SegmentStateController()
        d = c.diagnostics()
        for key in (
            "sequence",
            "segment_counter",
            "active_segment_id",
            "active_segment_revision",
            "tracked_partial_segments",
            "tracked_partial_emit_segments",
        ):
            self.assertIn(key, d)


if __name__ == "__main__":
    unittest.main()

