from __future__ import annotations

import re

_WORD_SPLIT_RE = re.compile(r"\s+")


def normalize_transcript_text(value: str) -> str:
    return " ".join(str(value or "").split())


def split_words(text: str) -> list[str]:
    normalized = normalize_transcript_text(text)
    if not normalized:
        return []
    return [part for part in _WORD_SPLIT_RE.split(normalized) if part]


def should_emit_partial(
    *,
    new_text: str,
    previous_text: str,
    mode: str = "word_growth",
    min_new_words: int = 1,
    min_delta_chars: int = 0,
    coalescing_ms: int = 0,
    previous_emit_monotonic: float | None = None,
    now_monotonic: float | None = None,
) -> bool:
    """
    Decide whether to publish a partial transcript update.

    ``word_growth`` mirrors Browser Web Speech: emit when the cumulative hypothesis
    gains at least ``min_new_words`` new words, or when the last word is revised.
    ``char_delta`` preserves the legacy character-delta + coalescing gate.
    """
    new_norm = normalize_transcript_text(new_text)
    if not new_norm:
        return False

    prev_norm = normalize_transcript_text(previous_text)
    if new_norm == prev_norm:
        return False

    emit_mode = str(mode or "word_growth").strip().lower() or "word_growth"
    if emit_mode == "char_delta":
        return _should_emit_char_delta(
            new_norm=new_norm,
            prev_norm=prev_norm,
            min_delta_chars=max(0, int(min_delta_chars)),
            coalescing_ms=max(0, int(coalescing_ms)),
            previous_emit_monotonic=previous_emit_monotonic,
            now_monotonic=now_monotonic,
        )

    return _should_emit_word_growth(
        new_norm=new_norm,
        prev_norm=prev_norm,
        min_new_words=max(1, int(min_new_words or 1)),
    )


def _should_emit_char_delta(
    *,
    new_norm: str,
    prev_norm: str,
    min_delta_chars: int,
    coalescing_ms: int,
    previous_emit_monotonic: float | None,
    now_monotonic: float | None,
) -> bool:
    growth_chars = len(new_norm) - len(prev_norm)
    if not prev_norm:
        return True
    if min_delta_chars <= 0 and coalescing_ms <= 0:
        return True
    if growth_chars < 0:
        return True
    if min_delta_chars > 0 and growth_chars >= min_delta_chars:
        return True
    if (
        coalescing_ms > 0
        and previous_emit_monotonic is not None
        and now_monotonic is not None
        and growth_chars >= 0
        and growth_chars < min_delta_chars
    ):
        elapsed_ms = (now_monotonic - previous_emit_monotonic) * 1000.0
        if elapsed_ms >= coalescing_ms:
            return True
        return False
    return growth_chars > 0


def _should_emit_word_growth(*, new_norm: str, prev_norm: str, min_new_words: int) -> bool:
    new_words = split_words(new_norm)
    prev_words = split_words(prev_norm)

    if not prev_words:
        return len(new_words) >= min_new_words

    if len(new_words) < len(prev_words):
        return False

    if len(new_words) == len(prev_words):
        return new_words != prev_words

    if new_words[: len(prev_words)] != prev_words:
        return True

    added = len(new_words) - len(prev_words)
    return added >= min_new_words
