from __future__ import annotations

import asyncio
from datetime import datetime, timezone
from pathlib import Path
import time
from typing import Callable, Literal

from backend.core.asr_engine import AsrEngine
from backend.core.cache_manager import CacheManager
from backend.core.audio_capture import AudioCapture, RNNoiseRecognitionProcessor
from backend.core.browser_asr_gateway import BrowserAsrGateway
from backend.core.exporter import Exporter
from backend.core.obs_caption_output import ObsCaptionOutput
from backend.core.runtime.asr_mode_controller import AsrModeController
from backend.core.runtime.runtime_metrics_controller import RuntimeMetricsController
from backend.core.runtime.runtime_state_controller import RuntimeStateController
from backend.core.runtime.runtime_lifecycle_coordinator import RuntimeLifecycleCoordinator
from backend.core.runtime.runtime_reset_controller import RuntimeResetController
from backend.core.runtime.runtime_session_controller import RuntimeSessionController
from backend.core.runtime.browser_worker_state_controller import BrowserWorkerStateController
from backend.core.runtime.processing_tasks_controller import ProcessingTasksController
from backend.core.runtime.audio_capture_controller import AudioCaptureController
from backend.core.runtime.remote_audio_state_controller import RemoteAudioStateController
from backend.core.runtime.speech_source_state_controller import SpeechSourceStateController
from backend.core.runtime.runtime_stop_state_controller import RuntimeStopStateController
from backend.core.runtime.runtime_export_controller import RuntimeExportController
from backend.core.runtime.segment_state_controller import SegmentStateController
from backend.core.runtime.runtime_start_state_controller import RuntimeStartStateController
from backend.core.runtime.translation_runtime_controller import TranslationRuntimeController
from backend.core.runtime.output_fanout_controller import OutputFanoutController
from backend.core.runtime.local_asr_pipeline import LocalAsrPipeline
from backend.core.runtime.partial_emit_coordinator import PartialEmitCoordinator
from backend.core.runtime.transcript_controller import TranscriptController
# SpeechSourceFactory is intentionally not used: we select among concrete SpeechSource implementations directly.
from backend.core.runtime.browser_speech_source import BrowserSpeechSource, _BrowserHooks
from backend.core.runtime.browser_asr_operational_fsm import BrowserAsrOperationalFsm
from backend.core.runtime.browser_asr_recovery_policy import (
    BrowserAsrPolicyExecutor,
    BrowserAsrRecoveryPolicy,
)
from backend.core.runtime.remote_controller_speech_source import RemoteControllerSpeechSource, _RemoteControllerHooks
from backend.core.runtime.remote_worker_speech_source import RemoteWorkerSpeechSource, _RemoteWorkerHooks
from backend.core.runtime.local_parakeet_speech_source import LocalParakeetSpeechSource, _LocalParakeetHooks
from backend.core.runtime.runtime_orchestrator_browser_worker_mixin import (
    RuntimeOrchestratorBrowserWorkerMixin,
)
from backend.core.runtime.runtime_orchestrator_diagnostics_mixin import (
    RuntimeOrchestratorDiagnosticsMixin,
)
from backend.core.runtime.runtime_orchestrator_local_asr_mixin import (
    RuntimeOrchestratorLocalAsrMixin,
)
from backend.core.runtime.runtime_orchestrator_lifecycle_mixin import (
    RuntimeOrchestratorLifecycleMixin,
)
from backend.core.runtime.runtime_orchestrator_state_metrics_mixin import (
    RuntimeOrchestratorStateMetricsMixin,
)
from backend.core.runtime.runtime_orchestrator_remote_ingress_mixin import (
    RuntimeOrchestratorRemoteIngressMixin,
)
from backend.core.segment_queue import SegmentQueue
from backend.core.structured_runtime_logger import StructuredRuntimeLogger
from backend.core.runtime.subtitle_presentation_controller import SubtitlePresentationController
from backend.core.translation_engine import TranslationEngine
from backend.core.vad import VadEngine
from backend.models import (
    RuntimeState,
)
from backend.ws_manager import WebSocketManager


class RuntimeOrchestrator(
    RuntimeOrchestratorStateMetricsMixin,
    RuntimeOrchestratorRemoteIngressMixin,
    RuntimeOrchestratorBrowserWorkerMixin,
    RuntimeOrchestratorDiagnosticsMixin,
    RuntimeOrchestratorLocalAsrMixin,
    RuntimeOrchestratorLifecycleMixin,
):

    def __init__(
        self,
        ws_manager: WebSocketManager,
        *,
        config_getter: Callable[[], dict],
        cache_manager: CacheManager,
        export_dir: Path,
        models_dir: Path,
        structured_logger: StructuredRuntimeLogger | None = None,
    ) -> None:
        self.ws_manager = ws_manager
        self.config_getter = config_getter
        self._obs_caption_output = ObsCaptionOutput(config_getter)
        self.subtitle_router = SubtitlePresentationController(
            ws_manager,
            config_getter,
            completed_callback=self._handle_completed_export_record,
            presentation_callback=self._handle_obs_caption_payload,
        )
        self._state = RuntimeState()
        self._audio_capture: AudioCapture | None = None
        self._vad = VadEngine()
        self._segment_queue = SegmentQueue()
        self._runtime_loop: asyncio.AbstractEventLoop | None = None
        self._latest_runtime_status_message: str | None = None
        self._asr_engine = AsrEngine(
            models_dir=models_dir,
            config_getter=config_getter,
            runtime_status_callback=self._emit_asr_runtime_status,
        )
        self._models_dir = models_dir
        self._translation_engine = TranslationEngine(cache_manager)
        self._exporter = Exporter(export_dir)
        self._structured_runtime_logger = structured_logger
        self._capture_task: asyncio.Task | None = None
        self._asr_task: asyncio.Task | None = None
        self._remote_audio_queue: asyncio.Queue[bytes] | None = None
        self._device_id: str | None = None
        self._local_audio_device_id: str | None = None
        self._segment_state = SegmentStateController()
        self._partial_emit = PartialEmitCoordinator(
            self._segment_state,
            lambda: self._effective_realtime_settings,
        )
        self._metrics_controller = RuntimeMetricsController()
        self._effective_realtime_settings = dict(self._LEGACY_VAD_SETTINGS)
        self._effective_subtitle_lifecycle_settings = {
            "completed_block_ttl_ms": 4500,
            "completed_source_ttl_ms": 4500,
            "completed_translation_ttl_ms": 4500,
            "pause_to_finalize_ms": self._LEGACY_VAD_SETTINGS["finalization_hold_ms"],
            "allow_early_replace_on_next_final": True,
            "sync_source_and_translation_expiry": True,
            "hard_max_phrase_ms": self._LEGACY_VAD_SETTINGS["max_segment_ms"],
        }
        self._rnnoise_processor = RNNoiseRecognitionProcessor(sample_rate=self._asr_engine.sample_rate, channels=1)
        self._browser_asr_gateway = BrowserAsrGateway(structured_logger=structured_logger)
        self._browser_asr_fsm = BrowserAsrOperationalFsm(structured_logger=structured_logger)
        self._browser_asr_recovery_policy = BrowserAsrRecoveryPolicy()
        self._browser_transport_probe: Callable[[], bool] | None = None
        self._browser_asr_policy_executor = BrowserAsrPolicyExecutor(
            structured_logger=structured_logger,
            can_send_control=self._browser_policy_can_send,
        )
        self._asr_mode = AsrModeController(self.config_getter)
        self._remote_audio_connected = False
        self._remote_audio_session_id: str | None = None
        self._remote_audio_last_chunk_monotonic: float | None = None
        self._runtime_status_heartbeat_interval_ms = 1000
        self._state_controller = RuntimeStateController(
            ws_manager,
            metrics_getter=lambda: self._metrics_controller.metrics,
            metrics_setter=self._metrics_controller.set_metrics,
            increment_counter_metric=lambda key, amount: self._increment_counter_metric(key, amount),
            heartbeat_interval_ms=self._runtime_status_heartbeat_interval_ms,
        )
        self._output = OutputFanoutController(
            ws_manager,
            obs_caption_output=self._obs_caption_output,
            state_controller=self._state_controller,
        )
        self._browser_worker_state = BrowserWorkerStateController()
        self._asr_runtime_generation: int = 0
        self._in_flight_transcribe_count: int = 0
        self._segment_queued_audio_len: dict[str, int] = {}
        self._translation = TranslationRuntimeController(
            translation_engine=self._translation_engine,
            config_getter=self.config_getter,
            is_sequence_relevant_for_translation=self.subtitle_router.is_sequence_relevant_for_translation,
            handle_translation_event=self._publish_translation_dispatch_event,
            metrics_callback=self._apply_translation_dispatcher_metrics,
            structured_logger=structured_logger,
        )
        self._reset = RuntimeResetController(
            reset_vad=self._vad.reset,
            clear_segment_queue=self._segment_queue.clear,
            reset_asr_runtime_state=self._asr_engine.reset_runtime_state,
            reset_state_broadcast=self._state_controller.reset_broadcast_state,
            clear_partial_tracking=self._segment_state.clear_all_partial_tracking,
            reset_browser_worker_status_signature=lambda: self._browser_worker_state.clear_status_signature(),
        )
        self._session = RuntimeSessionController(
            bump_asr_runtime_generation=lambda: setattr(self, "_asr_runtime_generation", self._asr_runtime_generation + 1),
            set_sequence_zero=self._segment_state.reset_sequence,
            new_session_id=lambda: datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S-%f"),
            now_utc_iso=lambda: datetime.now(timezone.utc).isoformat(),
            now_monotonic=time.perf_counter,
            reset_metrics=self._metrics_controller.reset,
            reset_in_flight_transcribe_count=lambda: setattr(self, "_in_flight_transcribe_count", 0),
            clear_runtime_loop=lambda: setattr(self, "_runtime_loop", None),
        )
        self._remote_audio_state = RemoteAudioStateController(
            ensure_queue=self._ensure_remote_audio_queue,
            shutdown_queue=self._shutdown_remote_audio_queue,
            clear_queue=self._clear_remote_audio_queue,
            set_connected=lambda connected: setattr(self, "_remote_audio_connected", bool(connected)),
            set_session_id=lambda session_id: setattr(self, "_remote_audio_session_id", session_id),
            set_last_chunk_monotonic=lambda value: setattr(self, "_remote_audio_last_chunk_monotonic", value),
            now_monotonic=time.perf_counter,
        )
        self._start_state = RuntimeStartStateController(
            set_runtime_loop=lambda: setattr(self, "_runtime_loop", asyncio.get_running_loop()),
            clear_latest_status_message=lambda: setattr(self, "_latest_runtime_status_message", None),
            reset_metrics=self._metrics_controller.reset,
            reset_in_flight_transcribe_count=lambda: setattr(self, "_in_flight_transcribe_count", 0),
        )
        self._stop_state = RuntimeStopStateController(
            clear_latest_status_message=lambda: setattr(self, "_latest_runtime_status_message", None),
            bump_asr_runtime_generation=lambda: setattr(self, "_asr_runtime_generation", self._asr_runtime_generation + 1),
            set_idle_state=lambda: self._set_state(is_running=False, status="idle", started_at_utc=None, last_error=None),
        )
        self._export_ctl = RuntimeExportController(
            export_session_files=lambda stopped_at_utc: self._export_session_files(stopped_at_utc=stopped_at_utc),
        )
        async def _await_task(task: object) -> None:
            await task  # type: ignore[misc]

        self._local_asr_pipeline = LocalAsrPipeline(self)
        self._processing_tasks = ProcessingTasksController(
            create_capture_task=lambda: asyncio.create_task(self._local_asr_pipeline.run_capture_loop()),
            create_asr_task=lambda: asyncio.create_task(self._local_asr_pipeline.run_asr_loop()),
            await_task=_await_task,
        )
        self._audio_capture_ctl = AudioCaptureController(
            # Must be late-bound for tests that patch AudioCapture in this module.
            create_capture=lambda: AudioCapture(),
            stop_in_thread=lambda capture: asyncio.to_thread(capture.stop),
        )
        self._transcript = TranscriptController(
            subtitle=self.subtitle_router,
            translation=self._translation,
            output=self._output,
            publish_transcript=lambda event: self._broadcast_transcript(event),
            publish_source_event=self._output.publish_source_event,
            default_source_lang=str(self.config_getter().get("source_lang", "auto") or "auto"),
            config_getter=self.config_getter,
        )
        # NOTE: hook-based generic SpeechSource factory removed in favor of concrete SpeechSource implementations.
        self._browser_speech_source = BrowserSpeechSource(
            gateway=self._browser_asr_gateway,
            structured_logger=structured_logger,
            hooks=_BrowserHooks(
                browser_worker_connected=self._browser_asr_worker_connected_impl,
                browser_worker_disconnected=self._browser_asr_worker_disconnected_impl,
                update_browser_worker_status=self._update_browser_asr_worker_status_impl,
                build_partial_event=self._build_browser_partial_event,
                build_final_event=self._build_browser_final_event,
                transcript_sink_partial=self._handle_browser_partial_event,
                transcript_sink_final=self._handle_browser_final_event,
                browser_source_lang=self._browser_asr_source_lang,
                note_worker_event=lambda: self._increment_counter_metric("browser_worker_event_count", 1),
            ),
        )
        self._remote_controller_source = RemoteControllerSpeechSource(
            _RemoteControllerHooks(
                set_runtime_transcribing=lambda message: self._set_runtime_state(
                    is_running=True,
                    status="transcribing",
                    started_at_utc=self._state.started_at_utc,
                    status_message=message,
                ),
                set_runtime_translating=lambda message: self._set_runtime_state(
                    is_running=True,
                    status="translating",
                    started_at_utc=self._state.started_at_utc,
                    status_message=message,
                ),
                set_runtime_listening=lambda message: self._set_runtime_state(
                    is_running=True,
                    status="listening",
                    started_at_utc=self._state.started_at_utc,
                    status_message=message,
                ),
                transcript_sink=self._transcript.handle_event,
                handle_translation_event=self._publish_translation_dispatch_event,
                increment_final_metric=lambda: self._increment_metric("finals_emitted"),
            )
        )
        self._remote_worker_source = RemoteWorkerSpeechSource(
            _RemoteWorkerHooks(ingest_remote_audio_chunk=self._ingest_remote_audio_chunk_impl)
        )
        self._local_parakeet_source = LocalParakeetSpeechSource(
            _LocalParakeetHooks(
                start=self._start_local_parakeet_impl,
                stop=self._stop_local_parakeet_impl,
            )
        )
        self._speech_source_state = SpeechSourceStateController(
            get_active_source=lambda: getattr(self, "_active_speech_source", None),
            set_active_source=lambda source: setattr(self, "_active_speech_source", source),
            set_local_audio_device_id=lambda device_id: setattr(self, "_local_audio_device_id", device_id),
            set_device_id=lambda device_id: setattr(self, "_device_id", device_id),
            browser_source=self._browser_speech_source,
            remote_controller_source=self._remote_controller_source,
            remote_worker_source=self._remote_worker_source,
            local_parakeet_source=self._local_parakeet_source,
        )
        self._lifecycle = RuntimeLifecycleCoordinator(
            pre_start=lambda: self._start_state.pre_start(),
            pre_stop=lambda: self._stop_state.pre_stop(),
            start_translation=self._translation.start,
            stop_translation=self._translation.stop,
            start_obs_captions=self._obs_caption_output.start,
            stop_obs_captions=self._obs_caption_output.stop,
            apply_obs_settings=lambda: self._output.apply_live_settings(self.config_getter()),
            reset_subtitles=self.subtitle_router.reset,
            select_speech_source=lambda: self._speech_source_state.select_for_start(
                is_browser_mode=self._is_browser_asr_mode(self._current_asr_mode()),
                uses_remote_audio_source=self._uses_remote_audio_source(),
                uses_remote_event_source=self._uses_remote_event_source(),
            ),
            start_speech_source=(
                lambda: self._active_speech_source.start()
                if self._active_speech_source is not None
                else asyncio.sleep(0)
            ),
            stop_speech_source=(
                lambda: self._active_speech_source.stop()
                if self._active_speech_source is not None
                else asyncio.sleep(0)
            ),
            on_start_reset=self._on_runtime_start_reset,
            start_session=self._session.start_new_session,
            capture_asr_mode_for_start=lambda: self._asr_mode.capture_for_start(state_is_running=self._state.is_running),
            init_asr_runtime_if_needed=self._init_asr_runtime_if_needed,
            unload_asr_runtime_state=lambda: asyncio.to_thread(self._asr_engine.unload_runtime_state),
            safe_stop_audio=self._safe_stop_audio,
            shutdown_remote_audio=self._remote_audio_state.shutdown_for_stop,
            stop_session_cleanup=self._session.stop_cleanup,
            try_export_on_stop=lambda: self._export_ctl.try_export_on_stop()[1],
            broadcast_runtime=self._broadcast_runtime,
            clear_after_stop=lambda: self._speech_source_state.clear_after_stop(),
        )
        self._active_speech_source = None
        self._apply_vad_tuning()
        self._apply_recognition_processing_settings()
        self._translation.apply_live_settings()


    async def _set_listening_if_current(
        self,
        *expected_statuses: Literal["listening", "transcribing", "translating"],
        last_error: str | None = None,
        status_message: str | None = None,
        broadcast: bool = True,
    ) -> None:
        if not self._state.is_running or self._state.status not in expected_statuses:
            return
        if broadcast:
            await self._set_runtime_state(
                is_running=True,
                status="listening",
                started_at_utc=self._state.started_at_utc,
                last_error=last_error,
                status_message=status_message,
            )
            return
        self._state = self._state.model_copy(
            update={
                "running": True,
                "is_running": True,
                "phase": "listening",
                "status": "listening",
                "last_error": last_error,
                "status_message": status_message,
            }
        )

    async def _set_runtime_state(
        self,
        *,
        is_running: bool,
        status: Literal["idle", "starting", "listening", "transcribing", "translating", "error"],
        started_at_utc: str | None,
        last_error: str | None = None,
        status_message: str | None = None,
    ) -> None:
        self._set_state(
            is_running=is_running,
            status=status,
            started_at_utc=started_at_utc,
            last_error=last_error,
            status_message=status_message,
        )
        await self._broadcast_runtime()
