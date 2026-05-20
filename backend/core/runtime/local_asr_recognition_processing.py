"""RNNoise + PCM prep for local capture (delegated from RuntimeOrchestrator)."""

from __future__ import annotations

from typing import Callable

from backend.core.audio_capture import RNNoiseRecognitionProcessor
from backend.core.runtime.audio_runtime_controller import prepare_recognition_audio


def apply_recognition_processing_settings(
    *,
    config_getter: Callable[[], dict],
    rnnoise_processor: RNNoiseRecognitionProcessor,
) -> None:
    config = config_getter()
    asr_config = config.get("asr", {}) if isinstance(config, dict) else {}
    if not isinstance(asr_config, dict):
        asr_config = {}
    try:
        rnnoise_strength = int(asr_config.get("rnnoise_strength", 70) or 70)
    except (TypeError, ValueError):
        rnnoise_strength = 70
    rnnoise_processor.configure(
        enabled=bool(asr_config.get("rnnoise_enabled", asr_config.get("experimental_noise_reduction_enabled", False))),
        strength=rnnoise_strength,
    )


def prepare_recognition_audio_bytes(
    audio: bytes,
    *,
    config_getter: Callable[[], dict],
    rnnoise_processor: RNNoiseRecognitionProcessor,
) -> bytes:
    config = config_getter()
    asr_config = config.get("asr", {}) if isinstance(config, dict) else {}
    if not isinstance(asr_config, dict):
        asr_config = {}
    rnnoise_enabled = bool(
        asr_config.get("rnnoise_enabled", asr_config.get("experimental_noise_reduction_enabled", False))
    )
    return prepare_recognition_audio(
        audio,
        rnnoise_enabled=rnnoise_enabled,
        rnnoise_processor=rnnoise_processor,
    )
