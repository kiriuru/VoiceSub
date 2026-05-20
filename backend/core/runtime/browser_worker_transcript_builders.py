"""Pure builders for Browser Speech → TranscriptEvent / TranscriptSegment (worker path)."""

from __future__ import annotations

from backend.models import TranscriptEvent, TranscriptSegment


def build_browser_worker_transcript_segment(
    *,
    segment_id: str,
    revision: int,
    text: str,
    is_final: bool,
    source_lang: str,
    provider_name: str,
    sequence: int,
    asr_result_created_at_ms: int | None = None,
    worker_send_started_at_ms: int | None = None,
    worker_message_sequence: int | None = None,
    worker_generation_id: int | None = None,
    worker_session_id: str | None = None,
    backend_received_at_ms: int | None = None,
    backend_published_to_router_at_ms: int | None = None,
    asr_operational_event_id: str | None = None,
    causal_parent_asr_event_id: str | None = None,
) -> TranscriptSegment:
    return TranscriptSegment(
        segment_id=segment_id,
        text=text,
        is_partial=not is_final,
        is_final=is_final,
        start_ms=0,
        end_ms=0,
        source_lang=source_lang,
        provider=provider_name,
        latency_ms=0.0,
        sequence=sequence,
        revision=revision,
        asr_result_created_at_ms=asr_result_created_at_ms,
        worker_send_started_at_ms=worker_send_started_at_ms,
        worker_message_sequence=worker_message_sequence,
        worker_generation_id=worker_generation_id,
        worker_session_id=worker_session_id,
        backend_received_at_ms=backend_received_at_ms,
        backend_published_to_router_at_ms=backend_published_to_router_at_ms,
        asr_operational_event_id=asr_operational_event_id,
        causal_parent_asr_event_id=causal_parent_asr_event_id,
    )


def build_browser_segment_started_transcript_event(
    *,
    provider_name: str,
    sequence: int,
    segment_id: str,
    revision: int,
    source_lang: str,
) -> TranscriptEvent:
    device_id = f"{provider_name}_worker"
    return TranscriptEvent(
        event="partial",
        lifecycle_event="segment_started",
        text="",
        device_id=device_id,
        sequence=sequence,
        segment=TranscriptSegment(
            segment_id=segment_id,
            text="",
            is_partial=False,
            is_final=False,
            start_ms=0,
            end_ms=0,
            source_lang=source_lang,
            provider=provider_name,
            latency_ms=None,
            sequence=sequence,
            revision=revision,
        ),
    )


def build_browser_partial_transcript_event(
    *,
    partial_text: str,
    device_id: str,
    sequence: int,
    segment: TranscriptSegment,
    forced_final: bool,
) -> TranscriptEvent:
    return TranscriptEvent(
        event="partial",
        text=partial_text,
        device_id=device_id,
        sequence=sequence,
        lifecycle_event="partial_updated",
        segment=segment,
        forced_final=bool(forced_final),
    )


def build_browser_final_transcript_event(
    *,
    final_text: str,
    device_id: str,
    sequence: int,
    segment: TranscriptSegment,
    forced_final: bool,
) -> TranscriptEvent:
    return TranscriptEvent(
        event="final",
        text=final_text,
        device_id=device_id,
        sequence=sequence,
        lifecycle_event="segment_finalized",
        segment=segment,
        forced_final=bool(forced_final),
    )
