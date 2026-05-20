"""Runtime status and diagnostics surface on RuntimeOrchestrator (API / dashboard)."""

from __future__ import annotations

import time

from backend.core.asr_provider_selection import (
    BROWSER_GOOGLE_EXPERIMENTAL_MODE,
    DEFAULT_PARAKEET_PROVIDER,
    LOCAL_ASR_MODE as RESOLVED_LOCAL_ASR_MODE,
)
from backend.core.runtime.asr_diagnostics_assembler import (
    assemble_asr_diagnostics_on_error,
    assemble_browser_asr_diagnostics,
    assemble_local_parakeet_asr_diagnostics,
    build_browser_asr_provider_status,
)
from backend.models import AsrDiagnostics, ObsCaptionDiagnostics, RuntimeState, TranslationDiagnostics


class RuntimeOrchestratorDiagnosticsMixin:
    def status(self) -> RuntimeState:
        self._apply_vad_tuning()
        if self._uses_remote_audio_source():
            if self._remote_audio_last_chunk_monotonic is not None:
                self._record_metrics(
                    remote_audio_last_chunk_age_ms=(
                        time.perf_counter() - self._remote_audio_last_chunk_monotonic
                    )
                    * 1000.0
                )
        else:
            self._record_metrics(remote_audio_last_chunk_age_ms=None)
        self._state = self._build_runtime_state(
            is_running=self._state.is_running,
            status=self._state.status,
            started_at_utc=self._state.started_at_utc,
            last_error=self._state.last_error,
            status_message=self._state.status_message,
        )
        return self._state

    def asr_status(self):
        if self._is_browser_asr_mode():
            return build_browser_asr_provider_status(
                browser_mode=self._current_asr_mode(),
                external_worker_connected=self._browser_worker_state.external_worker_connected,
                is_runtime_running=self._state.is_running,
            )
        return self._asr_engine.status()

    def translation_diagnostics(self) -> TranslationDiagnostics:
        return self._translation.diagnostics()

    def obs_caption_diagnostics(self) -> ObsCaptionDiagnostics:
        return self._obs_caption_output.diagnostics()

    def asr_diagnostics(self) -> AsrDiagnostics:
        try:
            resolved_asr = self._resolved_asr_provider()
            if self._is_browser_asr_mode():
                browser_mode = self._current_asr_mode()
                browser_config = self._browser_asr_config()
                browser_lang = str(browser_config.get("recognition_language", "ru-RU") or "ru-RU")
                return assemble_browser_asr_diagnostics(
                    resolved_asr=resolved_asr,
                    browser_mode=browser_mode,
                    is_experimental_mode=browser_mode == BROWSER_GOOGLE_EXPERIMENTAL_MODE,
                    browser_lang=browser_lang,
                    browser_worker=self._browser_asr_gateway.diagnostics(),
                    external_worker_connected=self._browser_worker_state.external_worker_connected,
                    fallback_provider_preference=self._current_local_provider_preference(),
                    is_runtime_running=self._state.is_running,
                )
            diagnostics = self._asr_engine.diagnostics()
            rnnoise_status = self._rnnoise_processor.status()
            config = self.config_getter()
            asr_config = config.get("asr", {}) if isinstance(config, dict) else {}
            if not isinstance(asr_config, dict):
                asr_config = {}
            inference_mode_enabled = bool(getattr(self._asr_engine.provider, "inference_mode_enabled", False))
            realtime_cfg = asr_config.get("realtime", {})
            latency_preset_raw = (
                str(realtime_cfg.get("latency_preset", "") or "").strip() or None
                if isinstance(realtime_cfg, dict)
                else None
            )
            return assemble_local_parakeet_asr_diagnostics(
                resolved_local_mode=RESOLVED_LOCAL_ASR_MODE,
                resolved_asr=resolved_asr,
                engine_diagnostics=diagnostics,
                rnnoise_status=rnnoise_status,
                config_for_asr=asr_config,
                effective_realtime_settings=self._effective_realtime_settings,
                latency_preset_raw=latency_preset_raw,
                models_dir=self._models_dir,
                vad=self._vad,
                capture_sample_rate=self._audio_capture_ctl.sample_rate,
                engine_sample_rate=self._asr_engine.sample_rate,
                is_runtime_running=self._state.is_running,
                asr_runtime_generation=self._asr_runtime_generation,
                segment_queue=self._segment_queue,
                metrics_stale_partial_jobs_dropped=int(
                    self._metrics_controller.metrics.stale_partial_jobs_dropped or 0
                ),
                metrics_asr_stale_results_ignored=int(
                    self._metrics_controller.metrics.asr_stale_results_ignored or 0
                ),
                in_flight_transcribe_count=self._in_flight_transcribe_count,
                inference_mode_enabled=inference_mode_enabled,
            )
        except Exception as exc:
            return assemble_asr_diagnostics_on_error(
                resolved_local_mode=RESOLVED_LOCAL_ASR_MODE,
                exc=exc,
                default_parakeet_provider=DEFAULT_PARAKEET_PROVIDER,
                engine_sample_rate=self._asr_engine.sample_rate,
                vad=self._vad,
                effective_realtime_settings=self._effective_realtime_settings,
            )
