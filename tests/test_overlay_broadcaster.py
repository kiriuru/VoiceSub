from __future__ import annotations

import unittest
from unittest import mock

from backend.core.overlay_broadcaster import OverlayBroadcaster
from backend.models import SubtitlePayloadEvent


class _RecordingWsManager:
    def __init__(self) -> None:
        self.messages: list[dict] = []

    async def broadcast(self, message: dict) -> None:
        self.messages.append(message)


class OverlayBroadcasterTests(unittest.IsolatedAsyncioTestCase):
    async def test_partial_only_payload_not_time_deduped(self) -> None:
        ws = _RecordingWsManager()
        broadcaster = OverlayBroadcaster(ws)
        payload = SubtitlePayloadEvent(
            sequence=1,
            lifecycle_state="partial_only",
            active_partial_text="hello",
            show_source=True,
        )

        await broadcaster.publish(payload)
        await broadcaster.publish(payload)

        self.assertEqual(len(ws.messages), 2)

    async def test_completed_only_payload_can_be_deduped(self) -> None:
        ws = _RecordingWsManager()
        broadcaster = OverlayBroadcaster(ws)
        payload = SubtitlePayloadEvent(
            sequence=2,
            lifecycle_state="completed_only",
            completed_block_visible=True,
            visible_items=[],
        )

        with mock.patch("backend.core.overlay_broadcaster.time.perf_counter", side_effect=[100.0, 100.1]):
            await broadcaster.publish(payload)
            await broadcaster.publish(payload)

        self.assertEqual(len(ws.messages), 1)

    async def test_completed_only_payload_can_repeat_after_cooldown(self) -> None:
        ws = _RecordingWsManager()
        broadcaster = OverlayBroadcaster(ws)
        payload = SubtitlePayloadEvent(
            sequence=2,
            lifecycle_state="completed_only",
            completed_block_visible=True,
            visible_items=[],
        )

        with mock.patch("backend.core.overlay_broadcaster.time.perf_counter", side_effect=[100.0, 100.5]):
            await broadcaster.publish(payload)
            await broadcaster.publish(payload)

        self.assertEqual(len(ws.messages), 2)
