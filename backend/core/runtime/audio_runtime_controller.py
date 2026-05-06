from __future__ import annotations

import asyncio
import audioop

from backend.core.audio_capture import RNNoiseRecognitionProcessor


def pcm16_rms_level(audio: bytes) -> float:
    payload = bytes(audio or b"")
    if not payload or len(payload) < 2:
        return 0.0
    try:
        return float(audioop.rms(payload, 2) / 32768.0)
    except audioop.error:
        return 0.0


def prepare_recognition_audio(
    audio: bytes,
    *,
    rnnoise_enabled: bool,
    rnnoise_processor: RNNoiseRecognitionProcessor,
) -> bytes:
    if not rnnoise_enabled:
        return bytes(audio)
    return rnnoise_processor.process_for_recognition(bytes(audio))


async def clear_async_queue(queue: asyncio.Queue[bytes] | None) -> None:
    if queue is None:
        return
    while True:
        try:
            queue.get_nowait()
        except asyncio.QueueEmpty:
            break
