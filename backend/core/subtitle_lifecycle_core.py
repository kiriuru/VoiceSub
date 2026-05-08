from __future__ import annotations

import asyncio
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
import time
from typing import Any, Awaitable, Callable, Literal

from backend.models import SubtitlePayloadEvent, TranscriptEvent, TranslationEvent


@dataclass(slots=True)
class SubtitleLifecycleCore:
    """
    Owns the subtitle lifecycle state machine: records, partial/completed promotion, TTL expiry, relevance.

    Presentation (payload building, style, ordering) is injected via callbacks.
    """

    config_getter: Callable[[], dict]
    build_payload: Callable[[int], SubtitlePayloadEvent | None]
    build_presentation_payload: Callable[[], SubtitlePayloadEvent]
    build_export_record: Callable[[int, SubtitlePayloadEvent], dict[str, object] | None]
    completed_callback: Callable[[dict], None] | None
    publish_callback: Callable[[], Awaitable[None]]
    increment_counter_metric: Callable[
        [Literal["overlay_stale_translation_suppressed", "overlay_payload_mismatch_count"], int], None
    ]

    _records: dict[int, dict] = None  # type: ignore[assignment]
    _active_partial: dict | None = None
    _completed_sequence: int | None = None
    _latest_final_sequence: int | None = None
    _completed_expires_at_utc: str | None = None
    _completed_expires_at_monotonic: float | None = None
    _completed_source_expires_at_monotonic: float | None = None
    _completed_translation_expires_at_monotonic: float | None = None
    _pending_final_sequence: int | None = None
    _expiry_task: asyncio.Task | None = None
    _exported_sequences: set[int] = None  # type: ignore[assignment]

    def __post_init__(self) -> None:
        self._records = {}
        self._exported_sequences = set()

    # ---- lifecycle config (kept in core; affects promotion/relevance rules) ----
    def _subtitle_lifecycle_config(self) -> dict:
        config = self.config_getter()
        lifecycle = config.get("subtitle_lifecycle", {}) if isinstance(config, dict) else {}
        if not isinstance(lifecycle, dict):
            lifecycle = {}
        completed_ttl_ms = max(500, int(lifecycle.get("completed_block_ttl_ms", 4500) or 4500))
        source_ttl_ms = max(500, int(lifecycle.get("completed_source_ttl_ms", completed_ttl_ms) or completed_ttl_ms))
        translation_ttl_ms = max(
            500, int(lifecycle.get("completed_translation_ttl_ms", completed_ttl_ms) or completed_ttl_ms)
        )
        return {
            "completed_block_ttl_ms": max(source_ttl_ms, translation_ttl_ms),
            "completed_source_ttl_ms": source_ttl_ms,
            "completed_translation_ttl_ms": translation_ttl_ms,
            "pause_to_finalize_ms": max(120, int(lifecycle.get("pause_to_finalize_ms", 700) or 700)),
            "allow_early_replace_on_next_final": bool(lifecycle.get("allow_early_replace_on_next_final", True)),
            "sync_source_and_translation_expiry": bool(lifecycle.get("sync_source_and_translation_expiry", True)),
            "keep_completed_translation_during_active_partial": bool(
                lifecycle.get("keep_completed_translation_during_active_partial", True)
            ),
            "hard_max_phrase_ms": max(1000, int(lifecycle.get("hard_max_phrase_ms", 12000) or 12000)),
        }

    def _translation_required_for_display(self) -> bool:
        config = self.config_getter()
        translation_config = config.get("translation", {}) if isinstance(config, dict) else {}
        subtitle_output = config.get("subtitle_output", {}) if isinstance(config, dict) else {}
        if not isinstance(translation_config, dict) or not isinstance(subtitle_output, dict):
            return False
        lines = translation_config.get("lines", [])
        enabled_lines = [
            line
            for line in lines
            if isinstance(line, dict)
            and line.get("enabled", True)
            and str(line.get("slot_id") or "").strip()
            and str(line.get("target_lang") or "").strip()
        ]
        return bool(
            translation_config.get("enabled")
            and subtitle_output.get("show_translations", True)
            and int(subtitle_output.get("max_translation_languages", 0) or 0) > 0
            and bool(enabled_lines)
        )

    # ---- public facade hooks ----
    @property
    def completed_expires_at_utc(self) -> str | None:
        return self._completed_expires_at_utc

    @property
    def active_partial(self) -> dict | None:
        return dict(self._active_partial) if self._active_partial else None

    def record_for_sequence(self, sequence: int) -> dict | None:
        record = self._records.get(sequence)
        return dict(record) if isinstance(record, dict) else None

    def current_completed_payload(self, *, hide_source: bool = False) -> SubtitlePayloadEvent | None:
        return self._current_completed_payload(hide_source=hide_source)

    async def reset(self) -> None:
        if self._expiry_task is not None:
            self._expiry_task.cancel()
            try:
                await self._expiry_task
            except asyncio.CancelledError:
                pass
            self._expiry_task = None
        self._records.clear()
        self._active_partial = None
        self._completed_sequence = None
        self._latest_final_sequence = None
        self._completed_expires_at_utc = None
        self._completed_expires_at_monotonic = None
        self._completed_source_expires_at_monotonic = None
        self._completed_translation_expires_at_monotonic = None
        self._pending_final_sequence = None
        self._exported_sequences.clear()
        await self.publish_callback()

    async def clear_active_partial(self) -> None:
        self._active_partial = None
        await self.publish_callback()

    async def republish_latest(self) -> None:
        await self.publish_callback()

    # ---- transcript/translation ingestion ----
    async def handle_transcript(self, event: TranscriptEvent) -> None:
        if event.event == "partial":
            segment = event.segment
            self._active_partial = {
                "sequence": event.sequence,
                "text": event.text,
                "source_lang": segment.source_lang if segment is not None else self.config_getter().get("source_lang", "auto"),
                "provider": segment.provider if segment is not None else None,
            }
            await self.publish_callback()
            return

        segment = event.segment
        duration_ms = None
        if segment is not None:
            if segment.start_ms is not None and segment.end_ms is not None:
                duration_ms = max(0, int(segment.end_ms) - int(segment.start_ms))
            elif segment.end_ms is not None:
                duration_ms = int(segment.end_ms)

        self._records[event.sequence] = {
            "sequence": event.sequence,
            "source_text": event.text,
            "source_lang": segment.source_lang if segment is not None else self.config_getter().get("source_lang", "auto"),
            "translations": {},
            "provider": segment.provider if segment is not None else None,
            "translation_received": not self._translation_required_for_display(),
            "duration_ms": duration_ms,
            "finalized_at_utc": datetime.now(timezone.utc).isoformat(),
            "finalized_at_monotonic": time.perf_counter(),
        }
        self._active_partial = {
            "sequence": event.sequence,
            "text": event.text,
            "source_lang": segment.source_lang if segment is not None else self.config_getter().get("source_lang", "auto"),
            "provider": segment.provider if segment is not None else None,
        }
        self._pending_final_sequence = event.sequence
        if self._latest_final_sequence is None or event.sequence > self._latest_final_sequence:
            self._latest_final_sequence = event.sequence
        self._promote_or_defer(event.sequence)
        await self.publish_callback()

    async def handle_translation(
        self,
        event: TranslationEvent,
        *,
        legacy_language_to_slot_map: Callable[[dict[str, Any]], dict[str, str]],
        translation_slot_map: Callable[[dict[str, Any]], dict[str, dict[str, Any]]],
    ) -> None:
        record = self._records.get(event.sequence)
        if record is None:
            record = {
                "sequence": event.sequence,
                "source_text": event.source_text,
                "source_lang": event.source_lang,
                "translations": {},
                "provider": event.provider,
                "translation_received": True,
                "duration_ms": None,
                "finalized_at_utc": None,
                "finalized_at_monotonic": None,
            }
            self._records[event.sequence] = record

        record["source_text"] = event.source_text
        record["source_lang"] = event.source_lang
        record["provider"] = event.provider

        config = self.config_getter()
        translation_config = config.get("translation", {}) if isinstance(config, dict) else {}
        language_to_slot = legacy_language_to_slot_map(translation_config) if isinstance(translation_config, dict) else {}

        translations = dict(record.get("translations", {}))
        for item in event.translations:
            slot_id = str(item.slot_id or "").strip().lower()
            if not slot_id:
                target_lang = str(item.target_lang or "").strip().lower()
                slot_id = language_to_slot.get(target_lang, "")
            translation_key = str(slot_id or item.target_lang).strip().lower()
            if not translation_key:
                continue
            translations[translation_key] = {
                "slot_id": slot_id or item.slot_id,
                "target_lang": item.target_lang,
                "label": item.label,
                "text": item.text,
                "provider": item.provider,
                "success": item.success,
                "error": item.error,
            }
        record["translations"] = translations

        required_slot_ids = list(translation_slot_map(translation_config).keys()) if isinstance(translation_config, dict) else []
        received_targets = {str(item).strip().lower() for item in translations.keys() if str(item).strip()}
        record["translation_received"] = bool(
            event.is_complete
            or not required_slot_ids
            or all(slot_id in received_targets for slot_id in required_slot_ids)
        )

        was_exported = event.sequence in self._exported_sequences
        should_promote = (
            self._pending_final_sequence == event.sequence
            or self._completed_sequence == event.sequence
            or (
                self._pending_final_sequence is None
                and self._completed_sequence is None
                and self._latest_final_sequence == event.sequence
            )
        )
        if should_promote:
            self._promote_or_defer(event.sequence)

        if was_exported and self._completed_sequence == event.sequence and self.completed_callback is not None:
            payload = self._promotion_payload(event.sequence)
            if payload is not None and payload.visible_items:
                export_record = self.build_export_record(event.sequence, payload)
                if export_record is not None:
                    self.completed_callback(export_record)

        await self.publish_callback()

    # ---- relevance ----
    def _completed_visibility(self, *, now_monotonic: float | None = None) -> tuple[bool, bool]:
        """
        Returns (source_visible, translation_visible) for the current completed sequence.

        Uses record.finalized_at_monotonic + configured TTLs as the source of truth to avoid
        drifting visibility decisions based on previously scheduled expiry timestamps.
        """
        if self._completed_sequence is None:
            return False, False
        record = self._records.get(self._completed_sequence)
        if record is None:
            return False, False
        finalized_at_monotonic = record.get("finalized_at_monotonic")
        if not isinstance(finalized_at_monotonic, (int, float)):
            return True, True

        lifecycle = self._subtitle_lifecycle_config()
        current = now_monotonic if now_monotonic is not None else time.perf_counter()
        source_ttl_ms = int(lifecycle["completed_source_ttl_ms"])
        translation_ttl_ms = int(lifecycle["completed_translation_ttl_ms"])

        payload = self.build_payload(self._completed_sequence)
        visible_items = list(payload.visible_items) if payload is not None else []
        has_visible_translation = any(
            item.kind == "translation" and item.visible and item.text for item in visible_items
        )
        if lifecycle["sync_source_and_translation_expiry"] and has_visible_translation:
            source_ttl_ms = max(source_ttl_ms, translation_ttl_ms)

        source_expiry = float(finalized_at_monotonic) + (source_ttl_ms / 1000.0)
        translation_expiry = float(finalized_at_monotonic) + (translation_ttl_ms / 1000.0)
        return current < source_expiry, current < translation_expiry

    def _source_ttl_expired_for_sequence(self, sequence: int, now_monotonic: float | None = None) -> bool:
        record = self._records.get(sequence)
        if record is None:
            return False
        finalized_at_monotonic = record.get("finalized_at_monotonic")
        if not isinstance(finalized_at_monotonic, (int, float)):
            return False
        lifecycle = self._subtitle_lifecycle_config()
        current = now_monotonic if now_monotonic is not None else time.perf_counter()
        source_expiry_monotonic = float(finalized_at_monotonic) + (int(lifecycle["completed_source_ttl_ms"]) / 1000.0)
        return current >= source_expiry_monotonic

    def _translation_ttl_expired_for_sequence(self, sequence: int, now_monotonic: float | None = None) -> bool:
        record = self._records.get(sequence)
        if record is None:
            return False
        finalized_at_monotonic = record.get("finalized_at_monotonic")
        if not isinstance(finalized_at_monotonic, (int, float)):
            return False
        lifecycle = self._subtitle_lifecycle_config()
        current = now_monotonic if now_monotonic is not None else time.perf_counter()
        translation_expiry_monotonic = float(finalized_at_monotonic) + (int(lifecycle["completed_translation_ttl_ms"]) / 1000.0)
        return current >= translation_expiry_monotonic

    def _sequence_awaits_translation(self, sequence: int | None) -> bool:
        if sequence is None or not self._translation_required_for_display():
            return False
        record = self._records.get(sequence)
        if record is None:
            return False
        if bool(record.get("translation_received")):
            return False
        if self._source_ttl_expired_for_sequence(sequence):
            return False
        return True

    def _sequence_can_accept_late_translation(self, sequence: int | None) -> bool:
        if sequence is None or not self._translation_required_for_display():
            return False
        record = self._records.get(sequence)
        if record is None:
            return False
        if bool(record.get("translation_received")):
            return False
        if self._translation_ttl_expired_for_sequence(sequence):
            return False
        return True

    def is_sequence_relevant_for_presentation(self, sequence: int) -> bool:
        if sequence not in self._records:
            return False
        if self._pending_final_sequence == sequence:
            return True
        if self._completed_sequence == sequence:
            if self._current_completed_payload() is not None:
                return True
            return self._sequence_can_accept_late_translation(sequence)
        if (
            self._completed_sequence is None
            and self._pending_final_sequence is None
            and self._latest_final_sequence == sequence
        ):
            payload = self._promotion_payload(sequence)
            return payload is not None and bool(payload.visible_items)
        return False

    def is_sequence_relevant_for_translation(self, sequence: int) -> bool:
        if sequence not in self._records:
            return False
        if self.is_sequence_relevant_for_presentation(sequence):
            return True
        if self._pending_final_sequence == sequence:
            return True
        if self._completed_sequence == sequence and self._sequence_awaits_translation(sequence):
            return True
        if self._latest_final_sequence == sequence and self._sequence_can_accept_late_translation(sequence):
            return True
        return False

    # ---- promotion/expiry ----
    def _promotion_payload(self, sequence: int) -> SubtitlePayloadEvent | None:
        payload = self.build_payload(sequence)
        if payload is None:
            return None
        if self._source_ttl_expired_for_sequence(sequence):
            # Translation-only remap when source TTL expired.
            remapped_items = []
            remapped_visible = []
            for item in payload.items:
                should_show = item.visible and bool(item.text) and item.kind != "source"
                updated = item.model_copy(update={"visible": should_show, "style_slot": item.style_slot if should_show else None})
                remapped_items.append(updated)
                if updated.visible and updated.text:
                    remapped_visible.append(updated)
            if not remapped_visible:
                return None
            line1 = remapped_visible[0].text if remapped_visible else ""
            line2 = "\n".join(item.text for item in remapped_visible[1:]) if len(remapped_visible) > 1 else ""
            return payload.model_copy(update={"items": remapped_items, "visible_items": remapped_visible, "line1": line1, "line2": line2})
        return payload

    def _current_completed_payload(self, *, hide_source: bool = False) -> SubtitlePayloadEvent | None:
        if self._completed_sequence is None:
            return None
        payload = self.build_payload(self._completed_sequence)
        if payload is None:
            return None
        now_monotonic = time.perf_counter()
        computed_source_visible, computed_translation_visible = self._completed_visibility(now_monotonic=now_monotonic)
        source_visible = computed_source_visible and not hide_source
        translation_visible = computed_translation_visible
        remapped_items = []
        remapped_visible = []
        for item in payload.items:
            should_show = item.visible and bool(item.text)
            if item.kind == "source":
                should_show = should_show and source_visible
            else:
                should_show = should_show and translation_visible
            updated = item.model_copy(update={"visible": should_show, "style_slot": item.style_slot if should_show else None})
            remapped_items.append(updated)
            if updated.visible and updated.text:
                remapped_visible.append(updated)
        if not remapped_visible:
            return None
        line1 = remapped_visible[0].text if remapped_visible else ""
        line2 = "\n".join(item.text for item in remapped_visible[1:]) if len(remapped_visible) > 1 else ""
        return payload.model_copy(update={"items": remapped_items, "visible_items": remapped_visible, "line1": line1, "line2": line2})

    def _can_promote(self, sequence: int) -> SubtitlePayloadEvent | None:
        if sequence not in self._records:
            return None
        payload = self._promotion_payload(sequence)
        if payload is None or not payload.visible_items:
            return None
        return payload

    def _promote_or_defer(self, sequence: int) -> None:
        payload = self._can_promote(sequence)
        if payload is None:
            self._pending_final_sequence = sequence
            return
        lifecycle = self._subtitle_lifecycle_config()
        if (
            self._completed_sequence is not None
            and self._completed_sequence != sequence
            and self._sequence_awaits_translation(self._completed_sequence)
        ):
            self._pending_final_sequence = sequence
            return
        if (
            self._completed_sequence is not None
            and self._completed_sequence != sequence
            and not lifecycle["allow_early_replace_on_next_final"]
        ):
            self._pending_final_sequence = sequence
            return
        preserved_pending = (
            self._pending_final_sequence if self._pending_final_sequence is not None and self._pending_final_sequence != sequence else None
        )
        self._completed_sequence = sequence
        self._pending_final_sequence = preserved_pending
        if self._active_partial and int(self._active_partial.get("sequence", -1)) == sequence:
            self._active_partial = None
        self._schedule_expiry(payload)

        if sequence not in self._exported_sequences:
            self._exported_sequences.add(sequence)
            export_record = self.build_export_record(sequence, payload)
            if export_record is not None and self.completed_callback is not None:
                self.completed_callback(export_record)

    def _schedule_expiry(self, payload: SubtitlePayloadEvent | None = None) -> None:
        if self._expiry_task is not None:
            self._expiry_task.cancel()
            self._expiry_task = None
        if self._completed_sequence is None:
            self._completed_expires_at_utc = None
            self._completed_expires_at_monotonic = None
            self._completed_source_expires_at_monotonic = None
            self._completed_translation_expires_at_monotonic = None
            return
        payload = payload or self.build_payload(self._completed_sequence)
        lifecycle = self._subtitle_lifecycle_config()
        visible_items = list(payload.visible_items) if payload is not None else []
        has_visible_source = any(item.kind == "source" and item.visible and item.text for item in visible_items)
        has_visible_translation = any(item.kind == "translation" and item.visible and item.text for item in visible_items)
        now_monotonic = time.perf_counter()
        now_utc = datetime.now(timezone.utc)
        source_ttl_ms = int(lifecycle["completed_source_ttl_ms"])
        translation_ttl_ms = int(lifecycle["completed_translation_ttl_ms"])
        if lifecycle["sync_source_and_translation_expiry"] and has_visible_translation:
            source_ttl_ms = max(source_ttl_ms, translation_ttl_ms)
        self._completed_source_expires_at_monotonic = (
            now_monotonic + (source_ttl_ms / 1000.0) if has_visible_source else now_monotonic - 0.001
        )
        self._completed_translation_expires_at_monotonic = (
            now_monotonic + (translation_ttl_ms / 1000.0) if has_visible_translation else now_monotonic - 0.001
        )
        self._schedule_next_expiry_check(now_monotonic=now_monotonic, now_utc=now_utc)

    def _schedule_next_expiry_check(self, *, now_monotonic: float | None = None, now_utc: datetime | None = None) -> None:
        expiry_points = [
            point
            for point in (self._completed_source_expires_at_monotonic, self._completed_translation_expires_at_monotonic)
            if point is not None and point > (now_monotonic if now_monotonic is not None else time.perf_counter())
        ]
        if not expiry_points:
            self._completed_expires_at_utc = None
            self._completed_expires_at_monotonic = None
            self._expiry_task = None
            return
        current_monotonic = now_monotonic if now_monotonic is not None else time.perf_counter()
        current_utc = now_utc if now_utc is not None else datetime.now(timezone.utc)
        self._completed_expires_at_monotonic = max(expiry_points)
        self._completed_expires_at_utc = (
            current_utc
            + timedelta(milliseconds=int(round((self._completed_expires_at_monotonic - current_monotonic) * 1000.0)))
        ).isoformat()
        loop = asyncio.get_running_loop()
        next_check_monotonic = min(expiry_points)
        sequence = self._completed_sequence
        self._expiry_task = loop.create_task(self._expire_completed_after(int(sequence or 0), next_check_monotonic))

    async def _expire_completed_after(self, sequence: int, check_monotonic: float) -> None:
        try:
            sleep_seconds = max(0.0, check_monotonic - time.perf_counter())
            await asyncio.sleep(sleep_seconds)
            if self._completed_sequence != sequence:
                return
            self._expiry_task = None
            payload = self._current_completed_payload()
            if payload is None and self._completed_sequence is not None:
                payload = self._promotion_payload(self._completed_sequence)
            if payload is not None:
                self._schedule_next_expiry_check()
                await self.publish_callback()
                return
            self._completed_sequence = None
            self._completed_expires_at_monotonic = None
            self._completed_expires_at_utc = None
            self._completed_source_expires_at_monotonic = None
            self._completed_translation_expires_at_monotonic = None
            if self._pending_final_sequence is not None:
                self._promote_or_defer(self._pending_final_sequence)
            await self.publish_callback()
        except asyncio.CancelledError:
            raise

