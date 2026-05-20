"""Remote controller/worker ingress path on RuntimeOrchestrator (audio + relayed transcript/translation).

Logic is moved unchanged from :class:`RuntimeOrchestrator`; behavior must stay aligned with frozen remote
surface (see AGENTS.md).
"""

from __future__ import annotations

import asyncio
import time

from backend.core.runtime.audio_runtime_controller import clear_async_queue
from backend.models import TranscriptEvent, TranslationEvent


class RuntimeOrchestratorRemoteIngressMixin:
    async def _clear_remote_audio_queue(self) -> None:
        await clear_async_queue(self._remote_audio_queue)

    async def _ensure_remote_audio_queue(self) -> None:
        if self._remote_audio_queue is None:
            self._remote_audio_queue = asyncio.Queue(maxsize=256)

    async def _shutdown_remote_audio_queue(self) -> None:
        await self._clear_remote_audio_queue()
        self._remote_audio_queue = None

    async def remote_audio_ingest_connected(self, *, session_id: str | None = None) -> None:
        self._remote_audio_state.note_connected(session_id=session_id)
        await self._set_listening_if_current(
            "listening",
            last_error=None,
            status_message="Remote controller audio stream is connected.",
        )

    async def remote_audio_ingest_disconnected(self) -> None:
        self._remote_audio_state.note_disconnected()
        await self._set_listening_if_current(
            "listening",
            last_error=None,
            status_message="Waiting for remote controller audio stream.",
        )

    async def ingest_remote_audio_chunk(self, payload: bytes) -> bool:
        source = getattr(self, "_active_speech_source", None)
        if source is not None:
            return bool(await source.ingest_remote_audio_chunk(payload))
        return bool(await self._ingest_remote_audio_chunk_impl(payload))

    async def _ingest_remote_audio_chunk_impl(self, payload: bytes) -> bool:
        if not self._state.is_running:
            return False
        if not self._uses_remote_audio_source():
            return False
        audio = bytes(payload or b"")
        if not audio:
            return False
        if len(audio) % 2 != 0:
            audio = audio[:-1]
            if not audio:
                return False
        remote_audio_queue = self._remote_audio_queue
        if remote_audio_queue is None:
            return False
        if remote_audio_queue.full():
            try:
                remote_audio_queue.get_nowait()
                self._increment_counter_metric("remote_audio_chunks_dropped", 1)
            except asyncio.QueueEmpty:
                pass
        await remote_audio_queue.put(audio)
        self._increment_counter_metric("remote_audio_chunks_in", 1)
        self._increment_counter_metric("remote_audio_bytes_in", len(audio))
        self._record_metrics(
            remote_audio_level_rms=self._pcm16_rms_level(audio),
            remote_audio_last_chunk_age_ms=0.0,
        )
        self._remote_audio_last_chunk_monotonic = time.perf_counter()
        return True

    async def ingest_remote_transcript_event(self, payload: dict) -> bool:
        source = getattr(self, "_active_speech_source", None)
        if source is not None:
            return bool(await source.ingest_remote_transcript_event(payload))
        return bool(await self._ingest_remote_transcript_event_impl(payload))

    async def _ingest_remote_transcript_event_impl(self, payload: dict) -> bool:
        if not self._state.is_running or not self._uses_remote_event_source():
            return False
        if not isinstance(payload, dict):
            return False
        try:
            event = TranscriptEvent.model_validate(payload)
        except Exception:
            return False
        if event.event == "partial":
            await self._set_runtime_state(
                is_running=True,
                status="transcribing",
                started_at_utc=self._state.started_at_utc,
                status_message="Receiving remote worker transcript stream.",
            )
        await self._transcript.handle_event(event)
        if event.event == "final":
            self._increment_metric("finals_emitted")
            await self._set_runtime_state(
                is_running=True,
                status="listening",
                started_at_utc=self._state.started_at_utc,
                status_message="Remote worker transcript stream is active.",
            )
        return True

    async def ingest_remote_translation_event(self, payload: dict) -> bool:
        source = getattr(self, "_active_speech_source", None)
        if source is not None:
            return bool(await source.ingest_remote_translation_event(payload))
        return bool(await self._ingest_remote_translation_event_impl(payload))

    async def _ingest_remote_translation_event_impl(self, payload: dict) -> bool:
        if not self._state.is_running or not self._uses_remote_event_source():
            return False
        if not isinstance(payload, dict):
            return False
        try:
            event = TranslationEvent.model_validate(payload)
        except Exception:
            return False
        await self._set_runtime_state(
            is_running=True,
            status="translating",
            started_at_utc=self._state.started_at_utc,
            status_message="Receiving remote worker translation stream.",
        )
        await self._broadcast_translation(event)
        await self.subtitle_router.handle_translation(event)
        await self._set_runtime_state(
            is_running=True,
            status="listening",
            started_at_utc=self._state.started_at_utc,
            status_message="Remote worker transcript stream is active.",
        )
        return True
