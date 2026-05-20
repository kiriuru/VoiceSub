from __future__ import annotations

import asyncio
import unittest

from backend.ws_manager import WebSocketManager


class _FakeWebSocket:
    def __init__(self, *, fail_with: type[BaseException] | None = None, send_delay_s: float = 0.0) -> None:
        self.fail_with = fail_with
        self.send_delay_s = float(send_delay_s)
        self.accepted = 0
        self.messages: list[dict] = []
        self._in_flight = 0
        self.max_in_flight_observed = 0

    async def accept(self) -> None:
        self.accepted += 1

    async def send_json(self, message: dict) -> None:
        if self.fail_with is not None:
            raise self.fail_with("boom")
        self._in_flight += 1
        if self._in_flight > self.max_in_flight_observed:
            self.max_in_flight_observed = self._in_flight
        try:
            if self.send_delay_s > 0:
                await asyncio.sleep(self.send_delay_s)
            self.messages.append(dict(message))
        finally:
            self._in_flight -= 1


class WebSocketManagerTests(unittest.TestCase):
    def test_broadcast_removes_dead_socket_and_keeps_live_socket(self) -> None:
        async def scenario() -> None:
            manager = WebSocketManager()
            live = _FakeWebSocket()
            dead = _FakeWebSocket(fail_with=OSError)
            await manager.connect(live)
            await manager.connect(dead)

            await manager.broadcast({"type": "runtime_update", "payload": {"status": "listening"}})
            await asyncio.sleep(0.05)

            self.assertEqual(len(live.messages), 1)
            self.assertEqual(manager.diagnostics()["ws_events_send_failures"], 1)
            self.assertEqual(manager.diagnostics()["ws_events_connections_active"], 1)

        asyncio.run(scenario())

    def test_duplicate_disconnect_is_idempotent(self) -> None:
        async def scenario() -> None:
            manager = WebSocketManager()
            socket = _FakeWebSocket()
            await manager.connect(socket)
            await manager.disconnect(socket)
            await manager.disconnect(socket)
            self.assertEqual(manager.diagnostics()["ws_events_connections_active"], 0)

        asyncio.run(scenario())

    def test_send_mutex_serializes_direct_and_queued_sends(self) -> None:
        async def scenario() -> None:
            manager = WebSocketManager()
            socket = _FakeWebSocket(send_delay_s=0.02)
            await manager.connect(socket)
            # Fire broadcast (queued path) and send_direct/replay_last concurrently;
            # the per-connection lock must guarantee no two send_json calls overlap.
            for i in range(5):
                await manager.broadcast({"type": "transcript_update", "payload": {"seq": i}})
            replay_task = asyncio.create_task(
                manager.replay_last(socket, message_types=["transcript_update"])
            )
            direct_task = asyncio.create_task(
                manager.send_direct(socket, {"type": "hello", "message": "connected"})
            )
            await asyncio.gather(replay_task, direct_task)
            # Wait for sender task to drain the queue.
            for _ in range(50):
                if len(socket.messages) >= 7:
                    break
                await asyncio.sleep(0.01)
            self.assertGreaterEqual(len(socket.messages), 5)
            self.assertEqual(socket.max_in_flight_observed, 1)
            await manager.disconnect(socket)

        asyncio.run(scenario())

    def test_send_direct_returns_false_when_socket_is_dead(self) -> None:
        async def scenario() -> None:
            manager = WebSocketManager()
            socket = _FakeWebSocket(fail_with=OSError)
            await manager.connect(socket)
            ok = await manager.send_direct(socket, {"type": "hello"})
            self.assertFalse(ok)
            self.assertEqual(manager.diagnostics()["ws_events_send_failures"], 1)
            self.assertEqual(manager.diagnostics()["ws_events_connections_active"], 0)

        asyncio.run(scenario())

    def test_queue_drops_oldest_on_pressure(self) -> None:
        async def scenario() -> None:
            manager = WebSocketManager(outbound_queue_max=2)
            # Slow socket so the sender task cannot drain the queue between pushes.
            socket = _FakeWebSocket(send_delay_s=0.05)
            await manager.connect(socket)
            for i in range(6):
                await manager.broadcast({"type": "transcript_update", "payload": {"seq": i}})
            await asyncio.sleep(0.4)
            await manager.disconnect(socket)
            # Drop-oldest means some early messages must be missing; the last one is preserved.
            self.assertLess(len(socket.messages), 6)
            self.assertGreaterEqual(manager.diagnostics()["ws_events_dropped_oldest"], 1)
            seqs = [msg.get("payload", {}).get("seq") for msg in socket.messages]
            self.assertIn(5, seqs)

        asyncio.run(scenario())

    def test_replay_last_uses_cached_message(self) -> None:
        async def scenario() -> None:
            manager = WebSocketManager()
            first = _FakeWebSocket()
            await manager.connect(first)
            await manager.broadcast({"type": "runtime_update", "payload": {"status": "listening"}})
            await asyncio.sleep(0.05)
            await manager.disconnect(first)

            second = _FakeWebSocket()
            await manager.connect(second)
            await manager.replay_last(second, message_types=["runtime_update"])
            self.assertEqual(len(second.messages), 1)
            self.assertEqual(second.messages[0]["type"], "runtime_update")

        asyncio.run(scenario())


if __name__ == "__main__":
    unittest.main()
