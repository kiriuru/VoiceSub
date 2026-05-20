from __future__ import annotations

import unittest

from backend.core.runtime.realtime_transcript_emit_policy import (
    normalize_transcript_text,
    should_emit_partial,
    split_words,
)


class RealtimeTranscriptEmitPolicyTests(unittest.TestCase):
    def test_normalize_collapses_whitespace(self) -> None:
        self.assertEqual(normalize_transcript_text("  hello   world  "), "hello world")

    def test_word_growth_requires_one_new_word(self) -> None:
        self.assertTrue(
            should_emit_partial(
                new_text="hello world",
                previous_text="hello",
                mode="word_growth",
                min_new_words=1,
            )
        )
        self.assertFalse(
            should_emit_partial(
                new_text="hello",
                previous_text="hello",
                mode="word_growth",
            )
        )
        self.assertFalse(
            should_emit_partial(
                new_text="hello",
                previous_text="hello world",
                mode="word_growth",
                min_new_words=1,
            )
        )

    def test_word_growth_allows_last_word_revision(self) -> None:
        self.assertTrue(
            should_emit_partial(
                new_text="hello worlds",
                previous_text="hello world",
                mode="word_growth",
            )
        )

    def test_word_growth_duplicate_suppressed(self) -> None:
        self.assertFalse(
            should_emit_partial(
                new_text="same phrase",
                previous_text="same phrase",
                mode="word_growth",
            )
        )

    def test_char_delta_legacy_gate(self) -> None:
        self.assertFalse(
            should_emit_partial(
                new_text="hello wor",
                previous_text="hello",
                mode="char_delta",
                min_delta_chars=12,
                coalescing_ms=160,
                previous_emit_monotonic=100.0,
                now_monotonic=100.05,
            )
        )

    def test_split_words_skips_empty(self) -> None:
        self.assertEqual(split_words("  a  b "), ["a", "b"])
