from __future__ import annotations

from typing import Any


async def broadcast_event(
    ws_manager: Any,
    *,
    channel: str,
    payload: dict[str, Any],
) -> None:
    await ws_manager.broadcast({"type": channel, "payload": payload})


async def publish_subtitle_payload(obs_caption_output: Any, payload: Any) -> None:
    await obs_caption_output.publish_subtitle_payload(payload)
