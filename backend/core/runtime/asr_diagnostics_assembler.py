"""
Pure assembly of `AsrDiagnostics` for runtime status / API.

Extracted from `RuntimeOrchestrator.asr_diagnostics` (Stage A refactor) so the
orchestrator stays thinner without changing the response shape.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Mapping

from backend.asr.parakeet.model_installer import (
    official_eu_parakeet_integrity_state,
    read_official_eu_parakeet_manifest,
)
from backend.core.audio_capture import RNNoiseStatus
from backend.core.asr_provider_selection import (
    BROWSER_GOOGLE_EXPERIMENTAL_MODE,
    DEFAULT_PARAKEET_PROVIDER,
)
from backend.core.parakeet_provider import (
    AsrProviderDiagnostics,
    AsrProviderStatus,
    OFFICIAL_EU_PARAKEET_REPO,
)
from backend.models import AsrDiagnostics
from backend.schemas.asr_schema import BrowserAsrDiagnostics


def build_browser_asr_provider_status(
    *,
    browser_mode: str,
    external_worker_connected: bool,
    is_runtime_running: bool,
) -> AsrProviderStatus:
    is_experimental = browser_mode == BROWSER_GOOGLE_EXPERIMENTAL_MODE
    if is_experimental:
        message = (
            "Experimental browser speech worker is connected."
            if external_worker_connected
            else "Experimental browser speech mode is configured. Open the browser worker window to capture audio."
        )
    else:
        message = (
            "Browser speech worker is connected."
            if external_worker_connected
            else "Browser speech mode is configured. Open the browser worker window to capture audio."
        )
    return AsrProviderStatus(
        provider=browser_mode,
        ready=True,
        message=message,
        requested_provider=browser_mode,
        requested_device_policy="browser_window",
        supports_gpu=False,
        supports_partials=True,
        supports_streaming=True,
        partials_supported=True,
        selected_device="browser",
        selected_execution_provider="webkitSpeechRecognition",
        runtime_initialized=is_runtime_running,
    )


def assemble_browser_asr_diagnostics(
    *,
    resolved_asr: Mapping[str, Any],
    browser_mode: str,
    is_experimental_mode: bool,
    browser_lang: str,
    browser_worker: BrowserAsrDiagnostics,
    external_worker_connected: bool,
    fallback_provider_preference: str,
    is_runtime_running: bool,
) -> AsrDiagnostics:
    worker_message = (
        "Experimental browser speech worker is connected."
        if external_worker_connected
        else "Open the experimental browser speech window and start recognition there."
    ) if is_experimental_mode else (
        "Browser speech worker is connected."
        if external_worker_connected
        else "Open the browser speech window and start recognition there."
    )
    return AsrDiagnostics(
        mode=str(resolved_asr.get("mode", browser_mode) or browser_mode),
        provider_preference=str(
            resolved_asr.get("provider_preference", fallback_provider_preference) or fallback_provider_preference
        ),
        effective_provider=str(resolved_asr.get("effective_provider", browser_mode) or browser_mode),
        provider=browser_mode,
        provider_label="Browser Google Speech Experimental"
        if is_experimental_mode
        else "Browser Google Speech",
        provider_kind=str(resolved_asr.get("provider_kind", "") or "") or "browser_worker",
        provider_mode_kind="browser_speech",
        uses_browser_worker=True,
        uses_backend_audio_capture=False,
        true_streaming=True,
        requested_provider=browser_mode,
        requested_device_policy="browser_window",
        requested_device="browser_window",
        cuda_available=False,
        supports_gpu=False,
        supports_partials=True,
        supports_streaming=True,
        gpu_requested=False,
        gpu_available=False,
        torch_built_with_cuda=False,
        torch_cuda_is_available=False,
        torch_device_count=0,
        degraded_mode=bool(browser_worker.degraded_reason),
        selected_device="browser",
        selected_execution_provider="webkitSpeechRecognition",
        partials_supported=True,
        sample_rate=None,
        recognition_noise_reduction_enabled=False,
        rnnoise_strength=0,
        rnnoise_available=False,
        rnnoise_active=False,
        rnnoise_message="RNNoise is not used in browser speech mode.",
        provider_phase=str(browser_worker.recognition_state or browser_worker.supervisor_state or "idle"),
        provider_message=worker_message,
        provider_error_kind=str(browser_worker.error_type or "") or None,
        provider_last_error=str(browser_worker.last_error or "") or None,
        message=(
            f"{worker_message} Recognition language: {browser_lang}. "
            "The worker may fall back to default start() if audio-track start is rejected."
            if is_experimental_mode
            else f"{worker_message} Recognition language: {browser_lang}."
        ),
        runtime_initialized=is_runtime_running,
        browser_worker=browser_worker,
    )


def assemble_local_parakeet_asr_diagnostics(
    *,
    resolved_local_mode: str,
    resolved_asr: Mapping[str, Any],
    engine_diagnostics: AsrProviderDiagnostics,
    rnnoise_status: RNNoiseStatus,
    config_for_asr: Mapping[str, Any],
    effective_realtime_settings: Mapping[str, Any],
    latency_preset_raw: str | None,
    models_dir: Path,
    vad: Any,
    capture_sample_rate: int | None,
    engine_sample_rate: int,
    is_runtime_running: bool,
    asr_runtime_generation: int,
    segment_queue: Any,
    metrics_stale_partial_jobs_dropped: int,
    metrics_asr_stale_results_ignored: int,
    in_flight_transcribe_count: int,
    inference_mode_enabled: bool,
) -> AsrDiagnostics:
    model_load_mode = str(config_for_asr.get("model_load_mode", "auto") or "auto") if config_for_asr else "auto"
    model_revision = str(config_for_asr.get("model_revision", "") or "") if config_for_asr else ""
    integrity_state, _integrity_detail = official_eu_parakeet_integrity_state(models_dir)
    manifest = read_official_eu_parakeet_manifest(models_dir)
    active_latency = str(latency_preset_raw or "").strip() or None

    return AsrDiagnostics(
        mode=str(resolved_asr.get("mode", resolved_local_mode) or resolved_local_mode),
        provider_preference=str(
            resolved_asr.get("provider_preference", engine_diagnostics.requested_provider or DEFAULT_PARAKEET_PROVIDER)
            or engine_diagnostics.requested_provider
            or DEFAULT_PARAKEET_PROVIDER
        ),
        effective_provider=str(
            resolved_asr.get("effective_provider", engine_diagnostics.provider_name) or engine_diagnostics.provider_name
        ),
        provider=engine_diagnostics.provider_name,
        provider_label="Official EU Parakeet Low Latency",
        provider_kind=str(resolved_asr.get("provider_kind", "") or "") or "local_parakeet",
        provider_mode_kind="local_ai",
        uses_browser_worker=False,
        uses_backend_audio_capture=True,
        true_streaming=engine_diagnostics.provider_name == "official_eu_parakeet_low_latency",
        requested_provider=engine_diagnostics.requested_provider,
        requested_device_policy=engine_diagnostics.requested_device_policy,
        requested_device="cuda" if engine_diagnostics.gpu_requested else "cpu",
        model_load_mode=model_load_mode,
        model_repo=OFFICIAL_EU_PARAKEET_REPO,
        model_revision=model_revision,
        model_path=engine_diagnostics.model_path,
        model_integrity_state=integrity_state if engine_diagnostics.model_path else "missing",
        model_loaded=bool(engine_diagnostics.model_loaded),
        model_manifest=manifest,
        supports_gpu=engine_diagnostics.supports_gpu,
        supports_partials=engine_diagnostics.supports_partials,
        supports_streaming=engine_diagnostics.supports_streaming,
        gpu_requested=engine_diagnostics.gpu_requested,
        gpu_available=engine_diagnostics.gpu_available,
        cuda_available=engine_diagnostics.torch_cuda_is_available,
        torch_version=engine_diagnostics.torch_version,
        torch_built_with_cuda=engine_diagnostics.torch_built_with_cuda,
        torch_cuda_is_available=engine_diagnostics.torch_cuda_is_available,
        torch_cuda_version=engine_diagnostics.torch_cuda_version,
        torch_device_count=engine_diagnostics.torch_device_count,
        first_gpu_name=engine_diagnostics.first_gpu_name,
        python_executable=engine_diagnostics.python_executable,
        venv_path=engine_diagnostics.venv_path,
        degraded_mode=engine_diagnostics.degraded_mode,
        fallback_reason=engine_diagnostics.fallback_reason,
        cpu_fallback_reason=engine_diagnostics.cpu_fallback_reason,
        selected_device=engine_diagnostics.actual_selected_device,
        device_active=engine_diagnostics.device_active,
        selected_execution_provider=engine_diagnostics.actual_execution_provider,
        partials_supported=engine_diagnostics.supports_partials,
        sample_rate=(capture_sample_rate or engine_sample_rate),
        audio_frame_duration_ms=getattr(vad, "frame_duration_ms", None),
        vad_mode=getattr(vad, "vad_mode", None),
        vad_partial_interval_ms=getattr(vad, "partial_interval_frames", 0) * getattr(vad, "frame_duration_ms", 0) or None,
        vad_min_speech_ms=getattr(vad, "min_speech_frames", 0) * getattr(vad, "frame_duration_ms", 0) or None,
        vad_first_partial_min_speech_ms=getattr(vad, "first_partial_min_speech_frames", 0)
        * getattr(vad, "frame_duration_ms", 0)
        or None,
        vad_silence_padding_ms=getattr(vad, "silence_hold_frames", 0) * getattr(vad, "frame_duration_ms", 0) or None,
        vad_finalization_hold_ms=getattr(vad, "finalization_hold_frames", 0) * getattr(vad, "frame_duration_ms", 0) or None,
        vad_max_segment_ms=getattr(vad, "max_segment_frames", 0) * getattr(vad, "frame_duration_ms", 0) or None,
        vad_energy_gate_enabled=bool(getattr(vad, "energy_gate_enabled", False)),
        vad_min_rms_for_recognition=float(getattr(vad, "min_rms_for_recognition", 0.0)),
        vad_min_voiced_ratio=float(getattr(vad, "min_voiced_ratio", 0.0)),
        vad_speech_attack_frames=int(getattr(vad, "speech_attack_frames", 0) or 0) or None,
        vad_speech_preroll_frames=int(getattr(vad, "speech_preroll_frames", 0) or 0) or None,
        realtime_chunk_window_ms=int(effective_realtime_settings.get("chunk_window_ms", 0) or 0),
        realtime_chunk_overlap_ms=int(effective_realtime_settings.get("chunk_overlap_ms", 0) or 0),
        partial_min_delta_chars=int(effective_realtime_settings.get("partial_min_delta_chars", 0) or 0),
        partial_coalescing_ms=int(effective_realtime_settings.get("partial_coalescing_ms", 0) or 0),
        active_latency_preset=active_latency,
        streaming_decode=bool(effective_realtime_settings.get("streaming_decode", True)),
        partial_emit_mode=str(effective_realtime_settings.get("partial_emit_mode", "") or "") or None,
        partial_min_new_words=int(effective_realtime_settings.get("partial_min_new_words", 1) or 1),
        recognition_noise_reduction_enabled=rnnoise_status.enabled,
        rnnoise_strength=rnnoise_status.strength,
        rnnoise_available=rnnoise_status.backend_available,
        rnnoise_active=rnnoise_status.active,
        rnnoise_backend=rnnoise_status.backend_name,
        rnnoise_uses_resample=rnnoise_status.uses_resample,
        rnnoise_input_sample_rate=rnnoise_status.input_sample_rate,
        rnnoise_processing_sample_rate=rnnoise_status.processing_sample_rate,
        rnnoise_frame_size_samples=rnnoise_status.frame_size_samples,
        rnnoise_message=rnnoise_status.message,
        message=engine_diagnostics.message,
        runtime_initialized=engine_diagnostics.runtime_initialized,
        provider_phase="listening" if is_runtime_running else "idle",
        provider_message=engine_diagnostics.message,
        provider_error_kind="cpu_fallback" if engine_diagnostics.cpu_fallback_reason else None,
        provider_last_error=engine_diagnostics.fallback_reason or engine_diagnostics.cpu_fallback_reason,
        parakeet_generation=asr_runtime_generation,
        asr_queue_depth=segment_queue.qsize(),
        asr_queue_max_size=segment_queue.maxsize,
        asr_partial_jobs_dropped=segment_queue.partial_jobs_dropped,
        partial_jobs_coalesced=segment_queue.partial_jobs_coalesced,
        stale_partial_jobs_dropped=metrics_stale_partial_jobs_dropped,
        finals_prioritized_count=segment_queue.finals_prioritized_count,
        asr_stale_results_ignored=metrics_asr_stale_results_ignored,
        in_flight_transcribe_count=in_flight_transcribe_count,
        inference_mode_enabled=inference_mode_enabled,
        gpu_memory_allocated_mb=engine_diagnostics.gpu_memory_allocated_mb,
        gpu_memory_reserved_mb=engine_diagnostics.gpu_memory_reserved_mb,
        gpu_peak_memory_allocated_mb=engine_diagnostics.gpu_peak_memory_allocated_mb,
        cuda_cache_cleared_count=int(engine_diagnostics.cuda_cache_cleared_count or 0),
        stream_states_count=engine_diagnostics.stream_states_count,
    )


def assemble_asr_diagnostics_on_error(
    *,
    resolved_local_mode: str,
    exc: Exception,
    default_parakeet_provider: str,
    engine_sample_rate: int,
    vad: Any,
    effective_realtime_settings: Mapping[str, Any],
) -> AsrDiagnostics:
    return AsrDiagnostics(
        mode=resolved_local_mode,
        provider_preference=default_parakeet_provider,
        effective_provider="unknown",
        provider="unknown",
        provider_label="Unknown ASR",
        provider_kind="unknown",
        provider_mode_kind="unknown",
        uses_browser_worker=False,
        uses_backend_audio_capture=False,
        true_streaming=False,
        requested_provider="unknown",
        requested_device_policy="unknown",
        requested_device="unknown",
        model_load_mode="auto",
        model_repo=OFFICIAL_EU_PARAKEET_REPO,
        model_revision="",
        model_integrity_state="unknown",
        supports_gpu=False,
        supports_partials=False,
        supports_streaming=False,
        cuda_available=False,
        torch_built_with_cuda=False,
        torch_cuda_is_available=False,
        torch_device_count=0,
        degraded_mode=True,
        fallback_reason=f"ASR diagnostics unavailable: {exc}",
        selected_device="unknown",
        selected_execution_provider="unknown",
        partials_supported=False,
        sample_rate=engine_sample_rate,
        audio_frame_duration_ms=getattr(vad, "frame_duration_ms", None),
        vad_mode=getattr(vad, "vad_mode", None),
        vad_partial_interval_ms=getattr(vad, "partial_interval_frames", 0) * getattr(vad, "frame_duration_ms", 0) or None,
        vad_min_speech_ms=getattr(vad, "min_speech_frames", 0) * getattr(vad, "frame_duration_ms", 0) or None,
        vad_first_partial_min_speech_ms=getattr(vad, "first_partial_min_speech_frames", 0)
        * getattr(vad, "frame_duration_ms", 0)
        or None,
        vad_silence_padding_ms=getattr(vad, "silence_hold_frames", 0) * getattr(vad, "frame_duration_ms", 0) or None,
        vad_finalization_hold_ms=getattr(vad, "finalization_hold_frames", 0) * getattr(vad, "frame_duration_ms", 0) or None,
        vad_max_segment_ms=getattr(vad, "max_segment_frames", 0) * getattr(vad, "frame_duration_ms", 0) or None,
        vad_energy_gate_enabled=bool(getattr(vad, "energy_gate_enabled", False)),
        vad_min_rms_for_recognition=float(getattr(vad, "min_rms_for_recognition", 0.0)),
        vad_min_voiced_ratio=float(getattr(vad, "min_voiced_ratio", 0.0)),
        vad_speech_attack_frames=int(getattr(vad, "speech_attack_frames", 0) or 0) or None,
        vad_speech_preroll_frames=int(getattr(vad, "speech_preroll_frames", 0) or 0) or None,
        realtime_chunk_window_ms=int(effective_realtime_settings.get("chunk_window_ms", 0) or 0),
        realtime_chunk_overlap_ms=int(effective_realtime_settings.get("chunk_overlap_ms", 0) or 0),
        message=f"ASR diagnostics unavailable: {exc}",
        provider_phase="error",
        provider_message="ASR diagnostics unavailable.",
        provider_error_kind=type(exc).__name__,
        provider_last_error=str(exc),
    )
