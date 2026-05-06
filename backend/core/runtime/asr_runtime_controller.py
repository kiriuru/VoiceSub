from __future__ import annotations

from typing import Any, Callable

from backend.core.asr_provider_selection import (
    BROWSER_GOOGLE_EXPERIMENTAL_MODE,
    BROWSER_GOOGLE_MODE,
    DEFAULT_PARAKEET_PROVIDER,
    LOCAL_ASR_MODE,
    resolve_effective_asr_provider,
)
from backend.core.remote_mode import (
    REMOTE_ROLE_CONTROLLER,
    REMOTE_ROLE_WORKER,
    resolve_configured_remote_state,
    resolve_effective_remote_role,
)


BROWSER_ASR_MODES = {BROWSER_GOOGLE_MODE, BROWSER_GOOGLE_EXPERIMENTAL_MODE}


def resolved_asr_provider(
    *,
    config_getter: Callable[[], dict],
    state_is_running: bool,
    active_runtime_mode: str | None,
    active_local_provider_preference: str | None,
) -> dict[str, Any]:
    config = config_getter()
    if state_is_running and active_runtime_mode in {LOCAL_ASR_MODE, *BROWSER_ASR_MODES}:
        config = dict(config if isinstance(config, dict) else {})
        asr = dict(config.get("asr", {}) if isinstance(config, dict) else {})
        asr["mode"] = active_runtime_mode
        if active_local_provider_preference is not None:
            asr["provider_preference"] = active_local_provider_preference
        config["asr"] = asr
    return resolve_effective_asr_provider(config)


def current_asr_mode(resolved_provider: dict[str, Any]) -> str:
    return str(resolved_provider.get("mode", LOCAL_ASR_MODE) or LOCAL_ASR_MODE)


def is_browser_asr_mode(mode: str | None = None) -> bool:
    return str(mode or LOCAL_ASR_MODE).strip().lower() in BROWSER_ASR_MODES


def current_local_provider_preference(resolved_provider: dict[str, Any]) -> str:
    return str(resolved_provider.get("provider_preference", DEFAULT_PARAKEET_PROVIDER) or DEFAULT_PARAKEET_PROVIDER)


def browser_asr_config(config: dict[str, Any]) -> dict[str, object]:
    asr = config.get("asr", {}) if isinstance(config, dict) else {}
    browser = asr.get("browser", {}) if isinstance(asr, dict) else {}
    return browser if isinstance(browser, dict) else {}


def browser_asr_source_lang(config: dict[str, Any]) -> str:
    language = str(browser_asr_config(config).get("recognition_language", "ru-RU") or "ru-RU").strip()
    primary = language.split("-", 1)[0].strip().lower()
    return primary or "auto"


def browser_worker_provider_name(mode: str) -> str:
    return mode if is_browser_asr_mode(mode) else BROWSER_GOOGLE_MODE


def current_remote_role(config_getter: Callable[[], dict]) -> str:
    try:
        return resolve_effective_remote_role(config_getter())
    except Exception:
        return "disabled"


def is_remote_enabled(config_getter: Callable[[], dict]) -> bool:
    enabled, _ = resolve_configured_remote_state(config_getter())
    return enabled


def uses_remote_audio_source(*, mode: str, remote_role: str) -> bool:
    return not is_browser_asr_mode(mode) and remote_role == REMOTE_ROLE_WORKER


def uses_remote_event_source(*, mode: str, remote_enabled: bool, remote_role: str) -> bool:
    return not is_browser_asr_mode(mode) and remote_enabled and remote_role == REMOTE_ROLE_CONTROLLER
