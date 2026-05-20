"""Local Parakeet capture/ASR helpers on RuntimeOrchestrator (VAD, partial emit, segment build)."""

from __future__ import annotations

from backend.core.runtime.local_asr_constants import LEGACY_VAD_SETTINGS
from backend.core.runtime.local_asr_hallucination_filter import should_drop_short_hallucination
from backend.core.runtime.local_asr_recognition_processing import (
    apply_recognition_processing_settings as apply_local_recognition_processing_settings,
    prepare_recognition_audio_bytes,
)
from backend.core.runtime.local_asr_vad_tuning import (
    apply_vad_tuning_from_settings,
    local_asr_streaming_delta_enqueue_enabled,
)
from backend.core.runtime.local_parakeet_transcript_segment import (
    build_local_parakeet_transcript_segment,
)
from backend.core.runtime.local_asr_realtime_settings import (
    resolve_realtime_settings,
    resolve_subtitle_lifecycle_settings,
)
from backend.core.runtime.segment_audio_enqueue import clear_segment_audio_enqueue_state
from backend.core.segment_queue import AsrWorkItem
from backend.models import TranscriptSegment


class RuntimeOrchestratorLocalAsrMixin:
    _LEGACY_VAD_SETTINGS = LEGACY_VAD_SETTINGS

    def _resolve_realtime_settings(self) -> dict[str, int | float | bool]:
        return resolve_realtime_settings(
            config_getter=self.config_getter,
            asr_engine=self._asr_engine,
            legacy_settings=self._LEGACY_VAD_SETTINGS,
        )

    def _resolve_subtitle_lifecycle_settings(self) -> dict[str, int | bool]:
        return resolve_subtitle_lifecycle_settings(
            config_getter=self.config_getter,
            legacy_settings=self._LEGACY_VAD_SETTINGS,
        )

    def _apply_recognition_processing_settings(self) -> None:
        apply_local_recognition_processing_settings(
            config_getter=self.config_getter,
            rnnoise_processor=self._rnnoise_processor,
        )

    def _prepare_recognition_audio(self, audio: bytes) -> bytes:
        return prepare_recognition_audio_bytes(
            audio,
            config_getter=self.config_getter,
            rnnoise_processor=self._rnnoise_processor,
        )

    def _apply_vad_tuning(self) -> None:
        settings = self._resolve_realtime_settings()
        lifecycle = self._resolve_subtitle_lifecycle_settings()
        apply_vad_tuning_from_settings(self._vad, realtime_settings=settings, lifecycle_settings=lifecycle)
        self._effective_realtime_settings = settings
        self._effective_subtitle_lifecycle_settings = lifecycle

    def _on_runtime_start_reset(self) -> None:
        self._reset.on_start_reset()
        clear_segment_audio_enqueue_state(self._segment_queued_audio_len)

    def _local_asr_delta_enqueue_enabled(self) -> bool:
        return local_asr_streaming_delta_enqueue_enabled(
            self.config_getter,
            is_browser_asr_mode=self._is_browser_asr_mode(),
        )

    def _should_emit_partial(self, segment_id: str, text: str) -> bool:
        return self._partial_emit.should_emit_partial(segment_id, text)

    def _should_drop_short_hallucination(self, *, text: str, duration_ms: int, is_final: bool) -> bool:
        return should_drop_short_hallucination(text=text, duration_ms=duration_ms, is_final=is_final)

    def _mark_partial_emitted(self, segment_id: str, text: str) -> None:
        self._partial_emit.mark_partial_emitted(segment_id, text)

    def _clear_partial_tracking(self, segment_id: str | None) -> None:
        self._partial_emit.clear_partial_tracking_for_segment(segment_id)

    def _next_sequence(self) -> int:
        return self._segment_state.next_sequence()

    def _assign_segment_tracking(self, kind: str, *, preferred_segment_id: str | None = None) -> tuple[str, int, bool]:
        _ = kind
        segment_id, revision, started_now, previous_to_clear = self._segment_state.assign_segment_tracking(
            preferred_segment_id=preferred_segment_id,
        )
        if previous_to_clear:
            self._clear_partial_tracking(previous_to_clear)
        return segment_id, revision, started_now

    def _build_transcript_segment(self, *, work_item: AsrWorkItem, text: str, latency_ms: float) -> TranscriptSegment:
        capabilities = self._asr_engine.capabilities()
        return build_local_parakeet_transcript_segment(
            work_item=work_item,
            text=text,
            latency_ms=latency_ms,
            segment_sequence=self._segment_state.sequence,
            source_lang=str(self.config_getter().get("source_lang", "auto")),
            provider_name=capabilities.provider_name,
        )
