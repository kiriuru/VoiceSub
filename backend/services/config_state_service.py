from __future__ import annotations

from copy import deepcopy
from dataclasses import dataclass
import hashlib
import json
import threading
from typing import Any, Literal

from fastapi import FastAPI


ActiveConfigSource = Literal["disk", "settings_save", "runtime_start_snapshot"]


@dataclass(frozen=True)
class ActiveConfigState:
    payload: dict[str, Any]
    source: ActiveConfigSource
    persisted: bool
    hash: str


class ConfigStateService:
    def __init__(self, app: FastAPI) -> None:
        self._app = app
        self._lock = threading.RLock()

    def current_payload(self) -> dict[str, Any]:
        with self._lock:
            payload = getattr(self._app.state, "config", {})
            return payload if isinstance(payload, dict) else {}

    def current_state(self) -> ActiveConfigState:
        with self._lock:
            state = getattr(self._app.state, "active_config_state", None)
            if isinstance(state, ActiveConfigState):
                return state
            fallback = self._build_state(self.current_payload(), source="disk", persisted=True)
            self._store_state(fallback)
            return fallback

    def set_loaded_from_disk(self, payload: dict[str, Any]) -> dict[str, Any]:
        return self._apply_payload(payload, source="disk", persisted=True, normalize=False)

    def set_settings_saved(self, payload: dict[str, Any]) -> dict[str, Any]:
        return self._apply_payload(payload, source="settings_save", persisted=True, normalize=False)

    def set_runtime_start_snapshot(self, payload: dict[str, Any]) -> dict[str, Any]:
        return self._apply_payload(payload, source="runtime_start_snapshot", persisted=False, normalize=True)

    def update_active_updates_metadata(self, *, latest_version: str | None, checked_utc: str) -> dict[str, Any]:
        """
        Patch only updates.* in the current active payload while preserving active_config_state.source.

        This is used by UpdateService to reflect the latest known version / check time in-memory
        without persisting an entire runtime_start_snapshot payload to disk.
        """
        with self._lock:
            state = self.current_state()
            payload = self._copy_payload(state.payload)
            updates = payload.get("updates", {})
            if not isinstance(updates, dict):
                updates = {}
            updates["latest_known_version"] = latest_version or ""
            updates["last_checked_utc"] = str(checked_utc or "").strip()
            payload["updates"] = updates
            patched = self._build_state(payload, source=state.source, persisted=state.persisted)
            self._store_state(patched)
            return patched.payload

    def _apply_payload(
        self,
        payload: dict[str, Any],
        *,
        source: ActiveConfigSource,
        persisted: bool,
        normalize: bool,
    ) -> dict[str, Any]:
        with self._lock:
            normalized = self._normalize_payload(payload) if normalize else self._copy_payload(payload)
            state = self._build_state(normalized, source=source, persisted=persisted)
            self._store_state(state)
            self._sync_remote_pairing_session(state.payload)
            return state.payload

    @staticmethod
    def _copy_payload(payload: dict[str, Any]) -> dict[str, Any]:
        return deepcopy(payload if isinstance(payload, dict) else {})

    def _normalize_payload(self, payload: dict[str, Any]) -> dict[str, Any]:
        copied = self._copy_payload(payload)
        config_manager = getattr(self._app.state, "config_manager", None)
        if config_manager is not None and hasattr(config_manager, "normalize_profile_payload"):
            return config_manager.normalize_profile_payload(copied)
        return copied

    def _build_state(
        self,
        payload: dict[str, Any],
        *,
        source: ActiveConfigSource,
        persisted: bool,
    ) -> ActiveConfigState:
        serialized = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        return ActiveConfigState(
            payload=payload,
            source=source,
            persisted=bool(persisted),
            hash=hashlib.sha256(serialized.encode("utf-8")).hexdigest(),
        )

    def _store_state(self, state: ActiveConfigState) -> None:
        # Caller holds self._lock.
        self._app.state.config = state.payload
        self._app.state.active_config_state = state

    def _sync_remote_pairing_session(self, payload: dict[str, Any]) -> None:
        manager = getattr(self._app.state, "remote_session_manager", None)
        if manager is None:
            return
        remote = payload.get("remote", {})
        if not isinstance(remote, dict):
            remote = {}
        manager.preload(
            session_id=str(remote.get("session_id", "") or "").strip() or None,
            pair_code=str(remote.get("pair_code", "") or "").strip() or None,
        )
