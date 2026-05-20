"""Pure builder for local Parakeet → TranscriptSegment (ASR work item path)."""

from __future__ import annotations

from backend.core.segment_queue import AsrWorkItem
from backend.models import TranscriptSegment


def build_local_parakeet_transcript_segment(
    *,
    work_item: AsrWorkItem,
    text: str,
    latency_ms: float,
    segment_sequence: int,
    source_lang: str,
    provider_name: str,
) -> TranscriptSegment:
    return TranscriptSegment(
        segment_id=work_item.segment_id or f"segment-{segment_sequence}",
        text=text,
        is_partial=work_item.kind == "partial",
        is_final=work_item.kind == "final",
        start_ms=0,
        end_ms=work_item.duration_ms,
        source_lang=source_lang,
        provider=provider_name,
        latency_ms=round(float(latency_ms), 2),
        sequence=segment_sequence,
        revision=work_item.revision,
    )
