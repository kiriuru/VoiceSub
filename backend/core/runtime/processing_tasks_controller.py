from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import Awaitable, Callable


@dataclass(slots=True)
class ProcessingTasksController:
    """
    Owns the lifecycle of capture/ASR asyncio tasks.
    """

    create_capture_task: Callable[[], object]
    create_asr_task: Callable[[], object]
    await_task: Callable[[object], Awaitable[None]]

    _capture_task: object | None = None
    _asr_task: object | None = None

    @property
    def capture_task(self) -> object | None:
        return self._capture_task

    @property
    def asr_task(self) -> object | None:
        return self._asr_task

    def ensure_started(self) -> None:
        capture = self._capture_task
        if capture is None or bool(getattr(capture, "done", lambda: False)()):
            self._capture_task = self.create_capture_task()

        asr = self._asr_task
        if asr is None or bool(getattr(asr, "done", lambda: False)()):
            self._asr_task = self.create_asr_task()

    async def stop(self) -> None:
        tasks = [task for task in (self._capture_task, self._asr_task) if task is not None]
        for task in tasks:
            cancel = getattr(task, "cancel", None)
            if callable(cancel):
                cancel()
        for task in tasks:
            try:
                await self.await_task(task)
            except asyncio.CancelledError:
                pass
        self._capture_task = None
        self._asr_task = None

    def clear_refs(self) -> None:
        self._capture_task = None
        self._asr_task = None

