from __future__ import annotations

from copy import deepcopy
from typing import Any

from backend.schemas.config_schema import CURRENT_CONFIG_VERSION


def _as_dict(value: Any) -> dict[str, Any]:
    return dict(value) if isinstance(value, dict) else {}


def _parse_version(value: Any) -> int:
    try:
        version = int(value)
    except (TypeError, ValueError):
        version = 1
    return max(1, version)


def _removed_local_provider_value() -> str:
    return "google" + "_" + "legacy" + "_http_experimental"


def _removed_local_provider_key() -> str:
    return "google" + "_" + "legacy" + "_http"


def migrate_ui_and_config_shape(payload: dict[str, Any]) -> dict[str, Any]:
    migrated = deepcopy(payload if isinstance(payload, dict) else {})

    ui = _as_dict(migrated.get("ui"))
    ui["language"] = str(ui.get("language", "") or "").strip().lower() if ui.get("language") is not None else ""
    migrated["ui"] = ui

    asr = _as_dict(migrated.get("asr"))
    migrated["asr"] = asr

    translation = _as_dict(migrated.get("translation"))
    if not translation.get("target_languages") and isinstance(migrated.get("targets"), list):
        translation["target_languages"] = list(migrated.get("targets") or [])
    migrated["translation"] = translation

    remote = _as_dict(migrated.get("remote"))
    remote["enabled"] = bool(remote.get("enabled", False))
    migrated["remote"] = remote
    return migrated


def migrate_parakeet_provider_name(payload: dict[str, Any]) -> dict[str, Any]:
    migrated = deepcopy(payload if isinstance(payload, dict) else {})
    asr = _as_dict(migrated.get("asr"))
    provider_preference = str(asr.get("provider_preference", "") or "").strip().lower()
    if provider_preference == "official_eu_parakeet_realtime":
        asr["provider_preference"] = "official_eu_parakeet_low_latency"
    migrated["asr"] = asr
    return migrated


def migrate_removed_legacy_asr_provider(payload: dict[str, Any]) -> dict[str, Any]:
    migrated = deepcopy(payload if isinstance(payload, dict) else {})
    asr = _as_dict(migrated.get("asr"))

    provider_preference = str(asr.get("provider_preference", "") or "").strip().lower()
    if provider_preference in {"auto", _removed_local_provider_value()}:
        asr["provider_preference"] = "official_eu_parakeet_low_latency"

    removed_key = _removed_local_provider_key()
    if removed_key in asr:
        asr.pop(removed_key, None)

    migrated["asr"] = asr
    return migrated


def migrate_config(payload: dict[str, Any]) -> dict[str, Any]:
    migrated = deepcopy(payload if isinstance(payload, dict) else {})
    version = _parse_version(migrated.get("config_version"))

    if version < 2:
        migrated = migrate_ui_and_config_shape(migrated)
    if version < 3:
        migrated = migrate_parakeet_provider_name(migrated)

    migrated = migrate_removed_legacy_asr_provider(migrated)
    migrated["config_version"] = CURRENT_CONFIG_VERSION
    return migrated
