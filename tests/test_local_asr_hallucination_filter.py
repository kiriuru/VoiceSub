from __future__ import annotations

import unittest

from backend.core.runtime.local_asr_hallucination_filter import should_drop_short_hallucination


class LocalAsrHallucinationFilterTests(unittest.TestCase):
    def test_drops_empty_text(self) -> None:
        self.assertTrue(should_drop_short_hallucination(text="", duration_ms=100, is_final=False))

    def test_drops_short_yeah_within_limit(self) -> None:
        self.assertTrue(should_drop_short_hallucination(text="yeah", duration_ms=500, is_final=False))

    def test_keeps_yeah_when_duration_too_long(self) -> None:
        self.assertFalse(should_drop_short_hallucination(text="yeah", duration_ms=2000, is_final=False))

    def test_keeps_non_token_text(self) -> None:
        self.assertFalse(should_drop_short_hallucination(text="hello world", duration_ms=200, is_final=True))


if __name__ == "__main__":
    unittest.main()
