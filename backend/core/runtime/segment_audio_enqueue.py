from __future__ import annotations


def slice_segment_audio_delta(
    *,
    segment_audio: bytes,
    segment_id: str,
    started_now: bool,
    queued_byte_len_by_segment: dict[str, int],
) -> tuple[bytes, bool]:
    """
    Given cumulative VAD segment PCM bytes, return only the suffix not yet enqueued.

    Returns ``(delta_audio, skip_enqueue)``. ``skip_enqueue`` is True when delta is empty.
    """
    key = str(segment_id or "").strip()
    if not key:
        return segment_audio, len(segment_audio) == 0

    if started_now:
        queued_byte_len_by_segment[key] = 0

    previous_len = int(queued_byte_len_by_segment.get(key, 0) or 0)
    total_len = len(segment_audio)
    if previous_len > total_len:
        queued_byte_len_by_segment[key] = 0
        previous_len = 0

    delta = segment_audio[previous_len:]
    queued_byte_len_by_segment[key] = total_len
    return delta, len(delta) == 0


def clear_segment_audio_enqueue_state(
    queued_byte_len_by_segment: dict[str, int],
    *,
    segment_id: str | None = None,
) -> None:
    if segment_id is None:
        queued_byte_len_by_segment.clear()
        return
    key = str(segment_id or "").strip()
    if key:
        queued_byte_len_by_segment.pop(key, None)
