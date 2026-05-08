from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import Any, Awaitable, Callable


@dataclass(slots=True)
class AudioCaptureController:
    """
    Owns AudioCapture start/stop and the stored instance.
    """

    create_capture: Callable[[], Any]
    stop_in_thread: Callable[[Any], Awaitable[None]]

    _device_id: str | None = None
    _capture: Any | None = None

    def set_device_id(self, device_id: str | None) -> None:
        self._device_id = str(device_id) if device_id is not None else None

    @property
    def capture(self) -> Any | None:
        return self._capture

    def start_if_needed(self) -> None:
        if self._capture is not None:
            return
        if self._device_id is None:
            return
        capture = self.create_capture()
        capture.start(device_id=self._device_id)
        self._capture = capture

    async def stop_if_running(self) -> None:
        if self._capture is None:
            return
        capture = self._capture
        self._capture = None
        try:
            await self.stop_in_thread(capture)
        except asyncio.CancelledError:
            raise

    async def read_chunk(self, seconds: float) -> bytes:
        if self._capture is None:
            return b""
        return await asyncio.to_thread(self._capture.read_chunk, float(seconds))

    @property
    def sample_rate(self) -> int | None:
        if self._capture is None:
            return None
        value = getattr(self._capture, "sample_rate", None)
        return int(value) if isinstance(value, (int, float)) else None

