from __future__ import annotations

import unittest

from backend.core.runtime.browser_worker_transcript_builders import (
    build_browser_final_transcript_event,
    build_browser_partial_transcript_event,
    build_browser_segment_started_transcript_event,
    build_browser_worker_transcript_segment,
)


class BrowserWorkerTranscriptBuildersTests(unittest.TestCase):
    def test_segment_and_events_match_worker_contract(self) -> None:
        seg = build_browser_worker_transcript_segment(
            segment_id="s1",
            revision=2,
            text="hello",
            is_final=False,
            source_lang="ru-RU",
            provider_name="browser_google",
            sequence=9,
            worker_generation_id=1,
        )
        self.assertTrue(seg.is_partial)
        self.assertEqual(seg.sequence, 9)
        self.assertEqual(seg.provider, "browser_google")

        partial_ev = build_browser_partial_transcript_event(
            partial_text="hello",
            device_id="browser_google_worker",
            sequence=9,
            segment=seg,
            forced_final=False,
        )
        self.assertEqual(partial_ev.event, "partial")
        self.assertEqual(partial_ev.lifecycle_event, "partial_updated")

        seg_final = build_browser_worker_transcript_segment(
            segment_id="s1",
            revision=2,
            text="hello",
            is_final=True,
            source_lang="ru-RU",
            provider_name="browser_google",
            sequence=10,
        )
        final_ev = build_browser_final_transcript_event(
            final_text="hello",
            device_id="browser_google_worker",
            sequence=10,
            segment=seg_final,
            forced_final=True,
        )
        self.assertEqual(final_ev.event, "final")
        self.assertEqual(final_ev.lifecycle_event, "segment_finalized")

    def test_segment_started_event_matches_worker_contract(self) -> None:
        ev = build_browser_segment_started_transcript_event(
            provider_name="browser_google",
            sequence=7,
            segment_id="s2",
            revision=1,
            source_lang="en-US",
        )
        self.assertEqual(ev.event, "partial")
        self.assertEqual(ev.lifecycle_event, "segment_started")
        self.assertEqual(ev.device_id, "browser_google_worker")
        self.assertEqual(ev.sequence, 7)
        self.assertEqual(ev.segment.segment_id, "s2")
        self.assertEqual(ev.segment.text, "")
        self.assertFalse(ev.segment.is_partial)
        self.assertFalse(ev.segment.is_final)
        self.assertIsNone(ev.segment.latency_ms)


if __name__ == "__main__":
    unittest.main()
