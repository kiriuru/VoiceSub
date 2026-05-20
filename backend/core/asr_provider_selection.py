from __future__ import annotations

from typing import Any


LOCAL_ASR_MODE = "local"
BROWSER_GOOGLE_MODE = "browser_google"
BROWSER_GOOGLE_EXPERIMENTAL_MODE = "browser_google_experimental"
DEFAULT_PARAKEET_PROVIDER = "official_eu_parakeet_low_latency"
VALID_LOCAL_PROVIDER_PREFERENCES = {
    "official_eu_parakeet_low_latency",
}
REMOVED_NON_LOW_LATENCY_PARAKEET = {
    "official_eu_parakeet",
}
REMOVED_LOCAL_PROVIDER_PREFERENCES = {
    "auto",
    "google" + "_" + "legacy" + "_http_experimental",
}


def _normalize_mode(raw_value: Any) -> str:
    normalized = str(raw_value or "").strip().lower()
    if normalized in {LOCAL_ASR_MODE, BROWSER_GOOGLE_MODE, BROWSER_GOOGLE_EXPERIMENTAL_MODE}:
        return normalized
    return LOCAL_ASR_MODE


def _normalize_provider_preference(raw_value: Any) -> str:
    normalized = str(raw_value or DEFAULT_PARAKEET_PROVIDER).strip().lower()
    if normalized in VALID_LOCAL_PROVIDER_PREFERENCES:
        return normalized
    if normalized in REMOVED_LOCAL_PROVIDER_PREFERENCES or normalized in REMOVED_NON_LOW_LATENCY_PARAKEET:
        return DEFAULT_PARAKEET_PROVIDER
    return DEFAULT_PARAKEET_PROVIDER


def resolve_effective_asr_provider(config: dict[str, Any] | None) -> dict[str, Any]:
    payload = config if isinstance(config, dict) else {}
    asr = payload.get("asr", {}) if isinstance(payload, dict) else {}
    if not isinstance(asr, dict):
        asr = {}

    mode = _normalize_mode(asr.get("mode"))
    provider_preference = _normalize_provider_preference(asr.get("provider_preference"))

    resolved = {
        "mode": mode,
        "provider_preference": provider_preference,
        "effective_provider": DEFAULT_PARAKEET_PROVIDER,
        "provider_label": "Local Parakeet ASR",
        "provider_kind": "local_parakeet",
        "uses_browser_worker": False,
        "uses_backend_audio_capture": True,
        "uses_parakeet": True,
        "warning": None,
    }

    if mode == BROWSER_GOOGLE_MODE:
        resolved.update(
            {
                "effective_provider": BROWSER_GOOGLE_MODE,
                "provider_label": "Browser Google Speech",
                "provider_kind": "browser_worker",
                "uses_browser_worker": True,
                "uses_backend_audio_capture": False,
                "uses_parakeet": False,
            }
        )
        return resolved

    if mode == BROWSER_GOOGLE_EXPERIMENTAL_MODE:
        resolved.update(
            {
                "effective_provider": BROWSER_GOOGLE_EXPERIMENTAL_MODE,
                "provider_label": "Browser Google Speech Experimental",
                "provider_kind": "browser_worker_experimental",
                "uses_browser_worker": True,
                "uses_backend_audio_capture": False,
                "uses_parakeet": False,
            }
        )
        return resolved

    resolved.update(
        {
            "effective_provider": provider_preference,
            "provider_label": "Local Parakeet ASR",
            "provider_kind": "local_parakeet",
            "uses_parakeet": True,
        }
    )
    return resolved
