from __future__ import annotations

from typing import Any

from backend.config.secrets import (
    normalize_google_translate_api_key,
    normalize_provider_secret,
    normalize_provider_text_value,
)


_SUPPORTED_PROVIDERS = {
    "google_translate_v2",
    "google_cloud_translation_v3",
    "google_gas_url",
    "google_web",
    "azure_translator",
    "deepl",
    "libretranslate",
    "openai",
    "openrouter",
    "lm_studio",
    "ollama",
    "public_libretranslate_mirror",
    "free_web_translate",
}


def normalize_provider_settings(payload: Any, *, defaults: dict[str, dict[str, str]]) -> dict[str, dict[str, str]]:
    if not isinstance(payload, dict):
        return defaults

    normalized: dict[str, dict[str, str]] = {}
    for provider_name, provider_defaults in defaults.items():
        current = payload.get(provider_name, {})
        if not isinstance(current, dict):
            current = {}
        normalized[provider_name] = {
            key: str(current.get(key, provider_defaults[key]))
            for key in provider_defaults
        }
        if provider_name == "google_translate_v2":
            normalized[provider_name]["api_key"] = normalize_google_translate_api_key(
                normalized[provider_name].get("api_key", "")
            )
        elif provider_name == "google_cloud_translation_v3":
            access_token_candidate = current.get("access_token", normalized[provider_name].get("access_token", ""))
            if not access_token_candidate:
                access_token_candidate = current.get("api_key", "")
            project_id_candidate = current.get("project_id", normalized[provider_name].get("project_id", ""))
            if not project_id_candidate:
                project_id_candidate = current.get("endpoint", "")
            location_candidate = current.get("location", normalized[provider_name].get("location", "global"))
            if not location_candidate:
                location_candidate = current.get("region", "global")
            normalized[provider_name]["project_id"] = normalize_provider_text_value(project_id_candidate)
            normalized[provider_name]["access_token"] = normalize_provider_secret(access_token_candidate)
            normalized[provider_name]["location"] = normalize_provider_text_value(location_candidate) or "global"
            normalized[provider_name]["model"] = normalize_provider_text_value(
                current.get("model", normalized[provider_name].get("model", ""))
            )
        else:
            for key in list(normalized[provider_name].keys()):
                value = normalized[provider_name][key]
                if key in {"api_key", "access_token"}:
                    normalized[provider_name][key] = normalize_provider_secret(value)
                else:
                    normalized[provider_name][key] = normalize_provider_text_value(value)
    return normalized


def normalize_translation_config(
    payload: Any,
    *,
    defaults: dict[str, Any],
    fallback_targets: Any,
) -> dict[str, Any]:
    translation = payload if isinstance(payload, dict) else {}
    provider = translation.get("provider", defaults["provider"])
    if provider not in _SUPPORTED_PROVIDERS:
        provider = defaults["provider"]

    target_languages = translation.get("target_languages", fallback_targets)
    if not isinstance(target_languages, list):
        target_languages = ["en"]

    try:
        translation_timeout_ms = int(translation.get("timeout_ms", defaults["timeout_ms"]) or defaults["timeout_ms"])
    except (TypeError, ValueError):
        translation_timeout_ms = int(defaults["timeout_ms"])
    try:
        translation_queue_max_size = int(
            translation.get("queue_max_size", defaults["queue_max_size"]) or defaults["queue_max_size"]
        )
    except (TypeError, ValueError):
        translation_queue_max_size = int(defaults["queue_max_size"])
    try:
        translation_max_concurrent_jobs = int(
            translation.get("max_concurrent_jobs", defaults["max_concurrent_jobs"]) or defaults["max_concurrent_jobs"]
        )
    except (TypeError, ValueError):
        translation_max_concurrent_jobs = int(defaults["max_concurrent_jobs"])

    return {
        "enabled": bool(translation.get("enabled", False)),
        "provider": provider,
        "target_languages": [str(item).lower() for item in target_languages if str(item).strip()],
        "timeout_ms": max(1000, min(60000, translation_timeout_ms)),
        "queue_max_size": max(1, min(64, translation_queue_max_size)),
        "max_concurrent_jobs": max(1, min(8, translation_max_concurrent_jobs)),
        "provider_settings": normalize_provider_settings(
            translation.get("provider_settings", {}),
            defaults=defaults["provider_settings"],
        ),
    }
