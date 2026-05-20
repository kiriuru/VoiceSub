from __future__ import annotations

import json
import time
from typing import Any

from backend.models import SubtitlePayloadEvent
from backend.ws_manager import WebSocketManager


class OverlayBroadcaster:
    def __init__(self, ws_manager: WebSocketManager) -> None:
        self.ws_manager = ws_manager
        self._last_payload_signature: str | None = None
        self._last_publish_monotonic: float = 0.0

    async def publish(self, payload: dict[str, Any] | SubtitlePayloadEvent) -> None:
        body = payload.model_dump() if isinstance(payload, SubtitlePayloadEvent) else payload
        body = dict(body)
        body.setdefault("event_type", "overlay_update")
        body["created_at_ms"] = int(time.time() * 1000)
        payload_signature = json.dumps(body, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        now_monotonic = time.perf_counter()
        lifecycle_state = str(body.get("lifecycle_state", "") or "").strip().lower()
        skip_time_dedupe = lifecycle_state in {"partial_only", "completed_with_partial"}
        # Shorter cooldown for stable completed states reduces visible overlay lag vs dashboard.
        signature_dedupe_cooldown_s = 0.45 if lifecycle_state == "completed_only" else 1.0
        if (
            not skip_time_dedupe
            and self._last_payload_signature == payload_signature
            and (now_monotonic - self._last_publish_monotonic) < signature_dedupe_cooldown_s
        ):
            return
        self._last_payload_signature = payload_signature
        self._last_publish_monotonic = now_monotonic
        await self.ws_manager.broadcast({"type": "overlay_update", "payload": body})
