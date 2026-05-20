"""Browser Speech worker path on RuntimeOrchestrator (transcript segment events + worker FSM hooks).

Kept as a mixin so :class:`RuntimeOrchestrator` stays a single public type while the core file shrinks.
"""

from __future__ import annotations

import time
from typing import Any

from backend.core.asr_provider_selection import BROWSER_GOOGLE_EXPERIMENTAL_MODE
from backend.core.runtime.browser_asr_trace import BrowserAsrTraceFields, new_event_id
from backend.core.runtime.browser_worker_transcript_builders import (
    build_browser_final_transcript_event,
    build_browser_partial_transcript_event,
    build_browser_segment_started_transcript_event,
    build_browser_worker_transcript_segment,
)
from backend.models import TranscriptEvent, TranscriptSegment


class RuntimeOrchestratorBrowserWorkerMixin:
    async def _broadcast_external_segment_started(
        self,
        *,
        segment_id: str,
        revision: int,
        source_lang: str,
    ) -> None:
        provider = self._browser_worker_provider_name()
        event = build_browser_segment_started_transcript_event(
            provider_name=provider,
            sequence=self._segment_state.sequence,
            segment_id=segment_id,
            revision=revision,
            source_lang=source_lang,
        )
        await self._broadcast_transcript_segment_event(event)

    async def _handle_browser_partial_event(self, event: TranscriptEvent) -> None:
        await self._transcript.handle_event(event)
        self._increment_metric("partial_updates_emitted")
        await self._set_listening_if_current(
            "listening",
            last_error=None,
            status_message="Browser speech recognition is active.",
            broadcast=False,
        )

    async def _handle_browser_final_event(self, event: TranscriptEvent) -> None:
        self._increment_metric("finals_emitted")
        await self._transcript.handle_event(event)
        self._segment_state.clear_active_segment()
        await self._set_listening_if_current(
            "listening",
            last_error=None,
            status_message="Browser speech recognition is active.",
            broadcast=False,
        )

    async def _build_browser_partial_event(
        self,
        *,
        partial_text: str,
        source_lang: str,
        client_segment_id: str | None,
        forced_final: bool,
        asr_result_created_at_ms: int | None,
        worker_send_started_at_ms: int | None,
        worker_message_sequence: int | None,
        worker_generation_id: int | None,
        worker_session_id: str | None,
        backend_received_at_ms: int | None,
        asr_operational_event_id: str | None = None,
        causal_parent_asr_event_id: str | None = None,
        basr_mono_ingress_at: float | None = None,
        transport_id: int | None = None,
    ) -> TranscriptEvent | None:
        if not self._state.is_running or not self._is_browser_asr_mode():
            return None
        if not partial_text:
            return None
        segment_id, revision, started_now = self._assign_segment_tracking("partial", preferred_segment_id=client_segment_id)
        if started_now:
            await self._broadcast_external_segment_started(
                segment_id=segment_id,
                revision=revision,
                source_lang=source_lang,
            )
        if not self._should_emit_partial(segment_id, partial_text):
            self._increment_metric("suppressed_partial_updates")
            return None
        self._mark_partial_emitted(segment_id, partial_text)
        self._next_sequence()
        backend_published_to_router_at_ms = int(time.time() * 1000)
        segment = self._build_external_transcript_segment(
            segment_id=segment_id,
            revision=revision,
            text=partial_text,
            is_final=False,
            source_lang=source_lang,
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
        self._maybe_note_browser_fsm_ingest(
            is_final=False,
            asr_operational_event_id=asr_operational_event_id,
            causal_parent_asr_event_id=causal_parent_asr_event_id,
            worker_generation_id=worker_generation_id,
            worker_session_id=worker_session_id,
            basr_mono_ingress_at=basr_mono_ingress_at,
            transport_id=transport_id,
        )
        provider = self._browser_worker_provider_name()
        return build_browser_partial_transcript_event(
            partial_text=partial_text,
            device_id=f"{provider}_worker",
            sequence=self._segment_state.sequence,
            segment=segment,
            forced_final=bool(forced_final),
        )

    async def _build_browser_final_event(
        self,
        *,
        final_text: str,
        source_lang: str,
        client_segment_id: str | None,
        forced_final: bool,
        asr_result_created_at_ms: int | None,
        worker_send_started_at_ms: int | None,
        worker_message_sequence: int | None,
        worker_generation_id: int | None,
        worker_session_id: str | None,
        backend_received_at_ms: int | None,
        asr_operational_event_id: str | None = None,
        causal_parent_asr_event_id: str | None = None,
        basr_mono_ingress_at: float | None = None,
        transport_id: int | None = None,
    ) -> TranscriptEvent | None:
        if not self._state.is_running or not self._is_browser_asr_mode():
            return None
        if not final_text:
            return None
        segment_id, revision, started_now = self._assign_segment_tracking("final", preferred_segment_id=client_segment_id)
        if started_now:
            await self._broadcast_external_segment_started(
                segment_id=segment_id,
                revision=revision,
                source_lang=source_lang,
            )
        self._clear_partial_tracking(segment_id)
        self._next_sequence()
        backend_published_to_router_at_ms = int(time.time() * 1000)
        segment = self._build_external_transcript_segment(
            segment_id=segment_id,
            revision=revision,
            text=final_text,
            is_final=True,
            source_lang=source_lang,
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
        self._maybe_note_browser_fsm_ingest(
            is_final=True,
            asr_operational_event_id=asr_operational_event_id,
            causal_parent_asr_event_id=causal_parent_asr_event_id,
            worker_generation_id=worker_generation_id,
            worker_session_id=worker_session_id,
            basr_mono_ingress_at=basr_mono_ingress_at,
            transport_id=transport_id,
        )
        provider = self._browser_worker_provider_name()
        return build_browser_final_transcript_event(
            final_text=final_text,
            device_id=f"{provider}_worker",
            sequence=self._segment_state.sequence,
            segment=segment,
            forced_final=bool(forced_final),
        )

    def _build_external_transcript_segment(
        self,
        *,
        segment_id: str,
        revision: int,
        text: str,
        is_final: bool,
        source_lang: str,
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
        return build_browser_worker_transcript_segment(
            segment_id=segment_id,
            revision=revision,
            text=text,
            is_final=is_final,
            source_lang=source_lang,
            provider_name=self._browser_worker_provider_name(),
            sequence=self._segment_state.sequence,
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

    def _maybe_note_browser_fsm_ingest(
        self,
        *,
        is_final: bool,
        asr_operational_event_id: str | None,
        causal_parent_asr_event_id: str | None,
        worker_generation_id: int | None,
        worker_session_id: str | None,
        basr_mono_ingress_at: float | None,
        transport_id: int | None,
    ) -> None:
        if not asr_operational_event_id:
            return
        trace = BrowserAsrTraceFields(
            event_id=asr_operational_event_id,
            causal_parent_id=causal_parent_asr_event_id,
            generation_id=worker_generation_id,
            session_id=worker_session_id,
            transport_id=transport_id,
            mono_ingress_at=basr_mono_ingress_at,
        )
        self._browser_asr_fsm.note_ingest(is_final=is_final, trace=trace)

    async def browser_asr_worker_connected(self) -> None:
        source = getattr(self, "_active_speech_source", None)
        if source is not None:
            await source.browser_worker_connected()
            return
        await self._browser_asr_worker_connected_impl()

    async def _browser_asr_worker_connected_impl(self) -> None:
        self._browser_worker_state.reset_for_start()
        self._browser_worker_state.mark_connected()
        browser_mode = self._current_asr_mode() if self._is_browser_asr_mode() else None
        self._browser_asr_gateway.worker_connected(browser_mode=browser_mode)
        conn_trace = BrowserAsrTraceFields(
            event_id=new_event_id(),
            causal_parent_id=None,
            generation_id=None,
            session_id=None,
            transport_id=None,
            mono_ingress_at=time.perf_counter(),
        )
        self._browser_asr_fsm.note_worker_connected(trace=conn_trace)
        if self._state.is_running and self._is_browser_asr_mode():
            await self._set_runtime_state(
                is_running=True,
                status="listening",
                started_at_utc=self._state.started_at_utc,
                last_error=None,
                status_message=(
                    "Experimental browser speech worker connected. Press Start Recognition in the popup window."
                    if browser_mode == BROWSER_GOOGLE_EXPERIMENTAL_MODE
                    else "Browser speech worker connected. Press Start Recognition in the popup window."
                ),
            )

    async def browser_asr_worker_disconnected(self) -> None:
        source = getattr(self, "_active_speech_source", None)
        if source is not None:
            await source.browser_worker_disconnected()
            return
        await self._browser_asr_worker_disconnected_impl()

    async def _browser_asr_worker_disconnected_impl(self) -> None:
        self._browser_worker_state.reset_for_stop()
        browser_mode = self._current_asr_mode() if self._is_browser_asr_mode() else None
        disc_trace = BrowserAsrTraceFields(
            event_id=new_event_id(),
            causal_parent_id=None,
            generation_id=None,
            session_id=None,
            transport_id=None,
            mono_ingress_at=time.perf_counter(),
        )
        self._browser_asr_gateway.worker_disconnected(browser_mode=browser_mode)
        self._browser_asr_fsm.note_worker_disconnected(trace=disc_trace)
        self._browser_asr_fsm.reset()
        self._segment_state.cleanup_on_browser_worker_disconnect()
        await self.subtitle_router.clear_active_partial()
        if self._state.is_running and self._is_browser_asr_mode():
            await self._set_runtime_state(
                is_running=True,
                status="listening",
                started_at_utc=self._state.started_at_utc,
                status_message=(
                    "Experimental browser speech worker disconnected. Reopen or restart the browser recognition window."
                    if browser_mode == BROWSER_GOOGLE_EXPERIMENTAL_MODE
                    else "Browser speech worker disconnected. Reopen or restart the browser recognition window."
                ),
            )

    async def update_browser_asr_worker_status(self, payload: dict[str, Any]) -> None:
        source = getattr(self, "_active_speech_source", None)
        if source is not None:
            await source.update_browser_worker_status(payload)
            return
        await self._update_browser_asr_worker_status_impl(payload)

    async def _update_browser_asr_worker_status_impl(self, payload: dict[str, Any]) -> None:
        previous = self._browser_asr_gateway.diagnostics()
        self._browser_asr_gateway.update_status(payload)
        current = self._browser_asr_gateway.diagnostics()
        st_event = str(payload.get("basr_status_event_id") or "").strip()
        status_trace: BrowserAsrTraceFields | None = None
        if st_event:
            tid = payload.get("basr_transport_id")
            mono = payload.get("basr_status_mono")
            status_trace = BrowserAsrTraceFields(
                event_id=st_event,
                causal_parent_id=None,
                generation_id=int(current.generation_id) if current.generation_id is not None else None,
                session_id=str(current.session_id).strip() if current.session_id else None,
                transport_id=int(tid) if tid is not None else None,
                mono_ingress_at=float(mono) if mono is not None else None,
            )
        if self._is_browser_asr_mode():
            self._browser_asr_fsm.note_status_aggregate(
                recognition_state=current.recognition_state,
                supervisor_state=current.supervisor_state,
                degraded_reason=str(current.degraded_reason).strip() if current.degraded_reason else None,
                worker_connected=bool(current.worker_connected),
                trace=status_trace,
            )
            actions = self._browser_asr_recovery_policy.suggest(
                degraded_reason=str(current.degraded_reason).strip() if current.degraded_reason else None,
                last_error=str(current.last_error).strip() if current.last_error else None,
                worker_connected=bool(current.worker_connected),
            )
            self._browser_asr_policy_executor.execute(actions=actions, trace=status_trace)
        if self._state.is_running and self._is_browser_asr_mode():
            self._increment_counter_metric("browser_worker_event_count", 1)
            signature = (
                current.worker_connected,
                current.desired_running,
                current.pending_start,
                current.recognition_state,
                current.supervisor_state,
                current.degraded_reason,
                current.last_error,
                current.generation_id,
                current.session_id,
                current.client_segment_id,
                current.forced_final,
                current.no_speech_count,
                current.network_error_count,
                current.duplicate_partial_suppressed,
                current.duplicate_final_suppressed,
                current.late_forced_final_suppressed,
                current.mic_track_ready_state,
                current.mic_track_muted,
                current.mic_rms,
                current.mic_active_recent_ms,
            )
            if signature == self._browser_worker_state.last_status_signature and previous.model_dump() == current.model_dump():
                self._increment_counter_metric("browser_worker_event_coalesced", 1)
                return
            self._browser_worker_state.update_status_signature(signature)
            await self._broadcast_runtime()

    async def ingest_external_asr_update(
        self,
        *,
        partial: str = "",
        final: str = "",
        is_final: bool = False,
        source_lang: str | None = None,
        generation_id: int | None = None,
        session_id: str | None = None,
        client_segment_id: str | None = None,
        forced_final: bool = False,
        asr_result_created_at_ms: int | None = None,
        worker_send_started_at_ms: int | None = None,
        worker_message_sequence: int | None = None,
        backend_received_at_ms: int | None = None,
        asr_operational_event_id: str | None = None,
        causal_parent_asr_event_id: str | None = None,
        basr_mono_ingress_at: float | None = None,
        transport_id: int | None = None,
    ) -> None:
        source = getattr(self, "_active_speech_source", None)
        if source is not None:
            await source.ingest_external_asr_update(
                partial=partial,
                final=final,
                is_final=is_final,
                source_lang=source_lang,
                generation_id=generation_id,
                session_id=session_id,
                client_segment_id=client_segment_id,
                forced_final=forced_final,
                asr_result_created_at_ms=asr_result_created_at_ms,
                worker_send_started_at_ms=worker_send_started_at_ms,
                worker_message_sequence=worker_message_sequence,
                backend_received_at_ms=backend_received_at_ms,
                asr_operational_event_id=asr_operational_event_id,
                causal_parent_asr_event_id=causal_parent_asr_event_id,
                basr_mono_ingress_at=basr_mono_ingress_at,
                transport_id=transport_id,
            )
            return
        return

    async def _ingest_external_asr_update_impl(
        self,
        *,
        partial: str = "",
        final: str = "",
        is_final: bool = False,
        source_lang: str | None = None,
        generation_id: int | None = None,
        session_id: str | None = None,
        client_segment_id: str | None = None,
        forced_final: bool = False,
        asr_result_created_at_ms: int | None = None,
        worker_send_started_at_ms: int | None = None,
        worker_message_sequence: int | None = None,
        backend_received_at_ms: int | None = None,
    ) -> None:
        _ = (
            partial,
            final,
            is_final,
            source_lang,
            generation_id,
            session_id,
            client_segment_id,
            forced_final,
            asr_result_created_at_ms,
            worker_send_started_at_ms,
            worker_message_sequence,
            backend_received_at_ms,
        )
        # Browser speech mode ingestion is owned by BrowserSpeechSource.
        return None
