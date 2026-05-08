from __future__ import annotations

import time
from typing import Any, Awaitable, Callable, Literal

from backend.core.asr_provider_selection import (
    BROWSER_GOOGLE_EXPERIMENTAL_MODE,
    BROWSER_GOOGLE_MODE,
    LOCAL_ASR_MODE as RESOLVED_LOCAL_ASR_MODE,
)
from backend.core.overlay_broadcaster import OverlayBroadcaster
from backend.core.subtitle_lifecycle_core import SubtitleLifecycleCore
from backend.core.subtitle_presentation import SubtitlePresentation
from backend.models import SubtitlePayloadEvent, TranscriptEvent, TranslationEvent
from backend.ws_manager import WebSocketManager

LOCAL_ASR_MODE = RESOLVED_LOCAL_ASR_MODE
BROWSER_ASR_MODE = BROWSER_GOOGLE_MODE
EXPERIMENTAL_BROWSER_ASR_MODE = BROWSER_GOOGLE_EXPERIMENTAL_MODE
BROWSER_ASR_MODES = {BROWSER_ASR_MODE, EXPERIMENTAL_BROWSER_ASR_MODE}


class SubtitleRouter:
    """
    Facade: publishes to overlay/WS, while delegating lifecycle state to SubtitleLifecycleCore
    and payload construction to SubtitlePresentation.
    """

    def __init__(
        self,
        ws_manager: WebSocketManager,
        config_getter: Callable[[], dict],
        completed_callback: Callable[[dict], None] | None = None,
        presentation_callback: Callable[[SubtitlePayloadEvent], Awaitable[None]] | None = None,
    ) -> None:
        self.ws_manager = ws_manager
        self.config_getter = config_getter
        self.completed_callback = completed_callback
        self.presentation_callback = presentation_callback
        self.overlay_broadcaster = OverlayBroadcaster(ws_manager)
        self._diagnostic_counters = {
            "overlay_stale_translation_suppressed": 0,
            "overlay_payload_mismatch_count": 0,
        }
        self._presentation = SubtitlePresentation(
            config_getter=self.config_getter,
            increment_counter_metric=lambda key, amount: self._increment_counter_metric(key, amount),
        )
        self._core = SubtitleLifecycleCore(
            config_getter=self.config_getter,
            build_payload=self._build_payload,
            build_presentation_payload=self._build_presentation_payload,
            build_export_record=self._build_export_record,
            completed_callback=self.completed_callback,
            publish_callback=self._publish_current,
            increment_counter_metric=lambda key, amount: self._increment_counter_metric(key, amount),
        )
        # Compatibility: some tests inspect internal records directly.
        self._records = self._core._records  # type: ignore[attr-defined]

    async def reset(self) -> None:
        await self._core.reset()

    def diagnostic_counters(self) -> dict[str, int]:
        return dict(self._diagnostic_counters)

    def _increment_counter_metric(
        self,
        key: Literal["overlay_stale_translation_suppressed", "overlay_payload_mismatch_count"],
        amount: int = 1,
    ) -> None:
        self._diagnostic_counters[key] = int(self._diagnostic_counters.get(key, 0) or 0) + int(amount)

    async def handle_transcript(self, event: TranscriptEvent) -> None:
        await self._core.handle_transcript(event)

    async def handle_translation(self, event: TranslationEvent) -> None:
        await self._core.handle_translation(
            event,
            legacy_language_to_slot_map=self._presentation.legacy_language_to_slot_map,
            translation_slot_map=self._presentation.translation_slot_map,
        )

    async def republish_latest(self) -> None:
        await self._core.republish_latest()

    async def clear_active_partial(self) -> None:
        await self._core.clear_active_partial()

    def is_sequence_relevant_for_presentation(self, sequence: int) -> bool:
        return self._core.is_sequence_relevant_for_presentation(sequence)

    def is_sequence_relevant_for_translation(self, sequence: int) -> bool:
        return self._core.is_sequence_relevant_for_translation(sequence)

    def _build_payload(self, sequence: int) -> SubtitlePayloadEvent | None:
        record = self._core.record_for_sequence(sequence)
        if record is None:
            return None
        return self._presentation.build_payload(sequence, record=record)

    def _build_export_record(self, sequence: int, payload: SubtitlePayloadEvent) -> dict[str, object] | None:
        record = self._core.record_for_sequence(sequence)
        if record is None:
            return None
        visible_items = [item.model_dump() for item in payload.visible_items]
        srt_text = "\n".join(item["text"] for item in visible_items if str(item.get("text", "")).strip())
        return {
            "type": "subtitle_record",
            "sequence": sequence,
            "source_text": str(record.get("source_text", "")),
            "source_lang": str(record.get("source_lang", "auto")),
            "provider": record.get("provider"),
            "duration_ms": record.get("duration_ms"),
            "finalized_at_utc": record.get("finalized_at_utc"),
            "finalized_at_monotonic": record.get("finalized_at_monotonic"),
            "translation_received": bool(record.get("translation_received")),
            "translations": dict(record.get("translations", {})),
            "display_order": list(payload.display_order),
            "items": [item.model_dump() for item in payload.items],
            "visible_items": visible_items,
            "srt_text": srt_text,
        }

    def _build_presentation_payload(self) -> SubtitlePayloadEvent:
        return self._presentation.build_presentation_payload(self._core)

    async def _publish_current(self) -> None:
        payload = self._build_presentation_payload()
        await self.overlay_broadcaster.publish(payload)
        # Mirror created_at_ms from overlay_update so dashboard ws clients can
        # detect freshness via timestamp; payload.sequence resets on every
        # runtime start and is unreliable as a stale-event guard on its own.
        subtitle_body = payload.model_dump()
        subtitle_body["created_at_ms"] = int(time.time() * 1000)
        await self.ws_manager.broadcast({"type": "subtitle_payload_update", "payload": subtitle_body})
        if self.presentation_callback is not None:
            await self.presentation_callback(payload)


def __getattr__(name: str) -> Any:
    # Compatibility shim for historical imports.
    if name == "RuntimeOrchestrator":
        from backend.core.runtime_orchestrator import RuntimeOrchestrator

        return RuntimeOrchestrator
    raise AttributeError(name)


__all__ = [
    "SubtitleRouter",
    "LOCAL_ASR_MODE",
    "BROWSER_ASR_MODE",
    "EXPERIMENTAL_BROWSER_ASR_MODE",
    "BROWSER_ASR_MODES",
    "RuntimeOrchestrator",
]

