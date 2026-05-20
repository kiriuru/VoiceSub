from __future__ import annotations

import unittest

from backend.core.runtime.local_parakeet_transcript_segment import build_local_parakeet_transcript_segment
from backend.core.segment_queue import AsrWorkItem


class LocalParakeetTranscriptSegmentTests(unittest.TestCase):
    def test_build_uses_work_item_and_sequence(self) -> None:
        wi = AsrWorkItem(
            kind="partial",
            audio=b"",
            duration_ms=500,
            segment_id="",
            revision=3,
        )
        seg = build_local_parakeet_transcript_segment(
            work_item=wi,
            text="hi",
            latency_ms=12.345,
            segment_sequence=10,
            source_lang="en-US",
            provider_name="official_eu_parakeet",
        )
        self.assertEqual(seg.segment_id, "segment-10")
        self.assertEqual(seg.text, "hi")
        self.assertTrue(seg.is_partial)
        self.assertFalse(seg.is_final)
        self.assertEqual(seg.end_ms, 500)
        self.assertEqual(seg.sequence, 10)
        self.assertEqual(seg.revision, 3)
        self.assertEqual(seg.provider, "official_eu_parakeet")
        self.assertEqual(seg.latency_ms, 12.35)

    def test_build_respects_explicit_segment_id(self) -> None:
        wi = AsrWorkItem(
            kind="final",
            audio=b"",
            duration_ms=0,
            segment_id="s-99",
            revision=1,
        )
        seg = build_local_parakeet_transcript_segment(
            work_item=wi,
            text="bye",
            latency_ms=0.0,
            segment_sequence=1,
            source_lang="auto",
            provider_name="mock",
        )
        self.assertEqual(seg.segment_id, "s-99")
        self.assertTrue(seg.is_final)


if __name__ == "__main__":
    unittest.main()
