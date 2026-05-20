"""Short-token hallucination drop policy for local Parakeet partial/final text."""

from __future__ import annotations

from backend.core.runtime.local_asr_constants import SHORT_HALLUCINATION_TOKENS


def should_drop_short_hallucination(*, text: str, duration_ms: int, is_final: bool) -> bool:
    normalized_text = " ".join(str(text or "").strip().split())
    if not normalized_text:
        return True

    lowered = normalized_text.casefold()
    word_count = len([part for part in lowered.replace("\n", " ").split(" ") if part.strip()])
    if lowered not in SHORT_HALLUCINATION_TOKENS:
        return False

    short_duration_limit_ms = 900 if is_final else 1100
    if duration_ms > short_duration_limit_ms:
        return False
    if word_count > 2:
        return False
    return True
