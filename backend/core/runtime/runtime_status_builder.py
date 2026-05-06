from __future__ import annotations

from typing import Any, Literal

from backend.models import (
    AsrDiagnostics,
    AsrRuntimeStatus,
    ObsCaptionDiagnostics,
    ObsCaptionsStatus,
    OverlayRuntimeStatus,
    RuntimeMetrics,
    RuntimeState,
    TranslationDiagnostics,
    TranslationRuntimeStatus,
)


def build_overlay_runtime_status(config: dict[str, Any]) -> OverlayRuntimeStatus:
    overlay = config.get("overlay", {}) if isinstance(config, dict) else {}
    subtitle_output = config.get("subtitle_output", {}) if isinstance(config, dict) else {}
    return OverlayRuntimeStatus(
        preset=str(overlay.get("preset", "single")) if isinstance(overlay, dict) else "single",
        compact=bool(overlay.get("compact", False)) if isinstance(overlay, dict) else False,
        show_source=bool(subtitle_output.get("show_source", True)) if isinstance(subtitle_output, dict) else True,
        show_translations=bool(subtitle_output.get("show_translations", True))
        if isinstance(subtitle_output, dict)
        else True,
        display_order=list(subtitle_output.get("display_order", [])) if isinstance(subtitle_output, dict) else [],
    )


def build_runtime_state(
    *,
    config: dict[str, Any],
    is_running: bool,
    status: Literal["idle", "starting", "listening", "transcribing", "translating", "error"],
    started_at_utc: str | None,
    last_error: str | None,
    status_message: str | None,
    metrics: RuntimeMetrics,
    subtitle_router_counters: dict[str, int],
    asr_diagnostics: AsrDiagnostics,
    translation_diagnostics: TranslationDiagnostics,
    obs_caption_diagnostics: ObsCaptionDiagnostics,
    resolved_asr: dict[str, Any],
    current_asr_mode: str,
    current_local_provider_preference: str,
    is_browser_asr_mode: bool,
) -> RuntimeState:
    asr_runtime = AsrRuntimeStatus(
        active_mode=str(resolved_asr.get("mode", current_asr_mode) or current_asr_mode),
        provider_preference=str(
            resolved_asr.get("provider_preference", current_local_provider_preference)
            or current_local_provider_preference
        ),
        effective_provider=str(resolved_asr.get("effective_provider", asr_diagnostics.provider) or asr_diagnostics.provider),
        provider=asr_diagnostics.provider,
        provider_label=asr_diagnostics.provider_label or asr_diagnostics.provider,
        provider_kind=str(resolved_asr.get("provider_kind", "") or "") or asr_diagnostics.provider_kind,
        uses_browser_worker=bool(resolved_asr.get("uses_browser_worker", False)),
        uses_backend_audio_capture=bool(resolved_asr.get("uses_backend_audio_capture", False)),
        provider_phase=asr_diagnostics.provider_phase or asr_diagnostics.provider_state,
        provider_message=asr_diagnostics.provider_message or asr_diagnostics.message,
        provider_error_kind=asr_diagnostics.provider_error_kind or asr_diagnostics.last_error_kind,
        provider_last_error=asr_diagnostics.provider_last_error or asr_diagnostics.last_error,
        ready=bool(asr_diagnostics.runtime_initialized or is_browser_asr_mode),
        true_streaming=bool(asr_diagnostics.true_streaming),
        supports_partials=bool(asr_diagnostics.supports_partials or asr_diagnostics.partials_supported),
        degraded_mode=bool(asr_diagnostics.degraded_mode),
        fallback_reason=asr_diagnostics.fallback_reason or asr_diagnostics.cpu_fallback_reason,
        diagnostics=asr_diagnostics,
    )
    translation_runtime = TranslationRuntimeStatus(
        enabled=translation_diagnostics.enabled,
        provider=translation_diagnostics.provider,
        ready=translation_diagnostics.ready,
        degraded_mode=translation_diagnostics.degraded,
        status=translation_diagnostics.status,
        summary=translation_diagnostics.summary,
        target_languages=list(translation_diagnostics.target_languages),
        diagnostics=translation_diagnostics,
    )
    obs_runtime = ObsCaptionsStatus(
        enabled=obs_caption_diagnostics.enabled,
        active=obs_caption_diagnostics.active,
        connected=obs_caption_diagnostics.connected,
        connection_state=obs_caption_diagnostics.connection_state,
        output_mode=obs_caption_diagnostics.output_mode,
        diagnostics=obs_caption_diagnostics,
    )
    degraded_mode = bool(asr_runtime.degraded_mode or translation_runtime.degraded_mode)
    fallback_reason = (
        asr_runtime.fallback_reason
        or translation_diagnostics.reason
        or translation_diagnostics.last_runtime_reason
        or last_error
    )
    return RuntimeState(
        running=is_running,
        starting=status == "starting",
        stopping=False,
        degraded_mode=degraded_mode,
        fallback_reason=fallback_reason,
        phase=status,
        is_running=is_running,
        status=status,
        started_at_utc=started_at_utc,
        last_error=last_error,
        status_message=status_message,
        asr=asr_runtime,
        translation=translation_runtime,
        overlay=build_overlay_runtime_status(config),
        obs_captions=obs_runtime,
        metrics=metrics.model_copy(update=subtitle_router_counters),
        asr_diagnostics=asr_diagnostics,
        translation_diagnostics=translation_diagnostics,
        obs_caption_diagnostics=obs_caption_diagnostics,
    )
