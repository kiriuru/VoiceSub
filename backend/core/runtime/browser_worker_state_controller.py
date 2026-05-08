from __future__ import annotations

from typing import Any


class BrowserWorkerStateController:
    """
    Owns Browser Speech worker connection/session/generation state.

    RuntimeOrchestrator should read this state from the controller instead of
    storing mirrored fields.
    """

    name = "browser_worker_state"

    def __init__(self) -> None:
        self.external_worker_connected: bool = False
        self.active_session_id: str | None = None
        self.active_generation_id: int = 0
        self.last_status_signature: tuple[Any, ...] | None = None

    def reset_for_start(self) -> None:
        self.external_worker_connected = False
        self.active_session_id = None
        self.active_generation_id = 0
        self.last_status_signature = None

    def reset_for_stop(self) -> None:
        self.external_worker_connected = False
        self.active_session_id = None
        self.active_generation_id = 0
        self.last_status_signature = None

    def mark_connected(
        self,
        *,
        session_id: str | None = None,
        generation_id: int | None = None,
    ) -> None:
        self.external_worker_connected = True
        if session_id is not None:
            self.active_session_id = str(session_id)
        if generation_id is not None:
            self.active_generation_id = int(generation_id)

    def mark_disconnected(self) -> None:
        self.external_worker_connected = False

    def update_session(
        self,
        *,
        session_id: str | None,
        generation_id: int | None,
    ) -> None:
        self.active_session_id = str(session_id) if session_id is not None else None
        if generation_id is not None:
            self.active_generation_id = int(generation_id)

    def clear_status_signature(self) -> None:
        self.last_status_signature = None

    def set_status_signature(self, signature: tuple[Any, ...] | None) -> None:
        self.last_status_signature = signature

    def update_status_signature(self, signature: tuple[Any, ...] | None) -> None:
        self.set_status_signature(signature)

    def diagnostics(self) -> dict[str, Any]:
        return {
            "external_worker_connected": self.external_worker_connected,
            "active_session_id": self.active_session_id,
            "active_generation_id": self.active_generation_id,
            "has_status_signature": self.last_status_signature is not None,
        }
