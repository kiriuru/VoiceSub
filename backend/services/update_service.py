from __future__ import annotations

from copy import deepcopy
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from typing import Any

import httpx
from fastapi import FastAPI

from backend.versioning import PROJECT_VERSION, build_version_info_payload, extract_latest_github_release_version


@dataclass(frozen=True)
class UpdateCheckResult:
    ok: bool
    latest_version: str | None
    last_checked_utc: str
    message: str


def _now_utc_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def _parse_checked_time(value: str | None) -> datetime | None:
    text = str(value or "").strip()
    if not text:
        return None
    try:
        parsed = datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


class UpdateService:
    """
    Live update checker for GitHub releases.

    The service is intentionally opt-in (`updates.enabled`) and time-bounded.
    It persists the latest known version + last check timestamp into config.json.
    """

    def __init__(self, app: FastAPI) -> None:
        self._app = app

    def _config_payload(self) -> dict[str, Any]:
        config_state_service = getattr(self._app.state, "config_state_service", None)
        if config_state_service is not None:
            payload = config_state_service.current_payload()
            return payload if isinstance(payload, dict) else {}
        payload = getattr(self._app.state, "config", {})
        return payload if isinstance(payload, dict) else {}

    def _persist_updates(self, *, latest_version: str | None, checked_utc: str) -> dict[str, Any]:
        config_manager = self._app.state.config_manager
        config_state_service = getattr(self._app.state, "config_state_service", None)

        # Protect runtime_start_snapshot: never persist the whole active in-memory payload
        # if it came from POST /api/runtime/start config_payload.
        if config_state_service is not None:
            state = config_state_service.current_state()
            if state.source == "runtime_start_snapshot":
                persisted_payload = config_manager.load()
                updates = persisted_payload.get("updates", {})
                if not isinstance(updates, dict):
                    updates = {}
                updates["latest_known_version"] = latest_version or ""
                updates["last_checked_utc"] = checked_utc
                persisted_payload["updates"] = updates
                config_manager.save(persisted_payload)
                # Keep the runtime snapshot active; only patch its updates metadata in-memory.
                return config_state_service.update_active_updates_metadata(
                    latest_version=latest_version,
                    checked_utc=checked_utc,
                )

        payload = deepcopy(self._config_payload())
        updates = payload.get("updates", {})
        if not isinstance(updates, dict):
            updates = {}
        updates["latest_known_version"] = latest_version or ""
        updates["last_checked_utc"] = checked_utc
        payload["updates"] = updates
        saved_payload = config_manager.save(payload)
        active_payload = (
            config_state_service.set_settings_saved(saved_payload) if config_state_service is not None else saved_payload
        )
        return active_payload

    async def check_now(self, *, force: bool = False) -> dict[str, Any]:
        config = self._config_payload()
        updates = config.get("updates", {}) if isinstance(config, dict) else {}
        updates = updates if isinstance(updates, dict) else {}

        enabled = bool(updates.get("enabled", False))
        github_repo = str(updates.get("github_repo", "") or "").strip()
        release_channel = str(updates.get("release_channel", "stable") or "stable").strip().lower()
        if release_channel not in {"stable", "prerelease"}:
            release_channel = "stable"

        try:
            interval_hours = int(updates.get("check_interval_hours", 12) or 12)
        except (TypeError, ValueError):
            interval_hours = 12
        interval_hours = max(1, min(168, interval_hours))

        payload = build_version_info_payload(config)
        if not enabled:
            payload["sync"]["message"] = "Update checks are disabled in settings."
            return payload
        if not github_repo:
            payload["sync"]["message"] = "Update checks are enabled, but updates.github_repo is not configured."
            return payload

        last_checked = _parse_checked_time(updates.get("last_checked_utc"))
        now = datetime.now(timezone.utc)
        due_at = None if last_checked is None else (last_checked + timedelta(hours=interval_hours))
        if not force and due_at is not None and now < due_at:
            payload["sync"]["message"] = "Update check skipped (interval not reached yet)."
            payload["sync"]["check_active"] = False
            return payload

        checked_utc = _now_utc_iso()
        payload["sync"]["check_active"] = True
        payload["sync"]["message"] = "Checking GitHub Releases..."

        api_url = f"https://api.github.com/repos/{github_repo}/releases?per_page=20"
        headers = {
            "Accept": "application/vnd.github+json",
            "User-Agent": f"stream-sub-translator/{PROJECT_VERSION}",
        }
        try:
            async with httpx.AsyncClient(timeout=httpx.Timeout(6.0, connect=3.0)) as client:
                response = await client.get(api_url, headers=headers)
                response.raise_for_status()
                releases = response.json()
        except Exception as exc:
            payload["sync"]["check_active"] = False
            payload["sync"]["message"] = f"Update check failed: {type(exc).__name__}: {exc}"
            return payload

        latest_version, selection_message = extract_latest_github_release_version(
            releases,
            release_channel=release_channel,
        )
        active_payload = self._persist_updates(latest_version=latest_version, checked_utc=checked_utc)
        payload = build_version_info_payload(active_payload)
        payload["sync"]["check_active"] = False
        payload["sync"]["message"] = selection_message
        return payload

