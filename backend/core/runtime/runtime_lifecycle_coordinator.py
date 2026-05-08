from __future__ import annotations

from dataclasses import dataclass
from typing import Awaitable, Callable


@dataclass(slots=True)
class RuntimeLifecycleCoordinator:
    """
    Centralizes ordered runtime lifecycle (start/stop) across the full runtime surface.

    Still callback-based, but now defines the canonical order for *all* runtime-owned components,
    not only translation/OBS/subtitles.
    """

    # Pre-start / pre-stop bookkeeping
    pre_start: Callable[[], None]
    pre_stop: Callable[[], None]

    # Core runtime surfaces
    start_translation: Callable[[], Awaitable[None]]
    stop_translation: Callable[[], Awaitable[None]]
    start_obs_captions: Callable[[], Awaitable[None]]
    stop_obs_captions: Callable[[], Awaitable[None]]
    apply_obs_settings: Callable[[], Awaitable[None]]
    reset_subtitles: Callable[[], Awaitable[None]]

    # Speech source + audio/tasks
    select_speech_source: Callable[[], None]
    start_speech_source: Callable[[], Awaitable[None]]
    stop_speech_source: Callable[[], Awaitable[None]]

    # Runtime session/reset/engine lifecycle
    on_start_reset: Callable[[], None]
    start_session: Callable[[], str]
    capture_asr_mode_for_start: Callable[[], None]
    init_asr_runtime_if_needed: Callable[[], Awaitable[None]]
    unload_asr_runtime_state: Callable[[], Awaitable[None]]

    # Stop-time cleanup
    safe_stop_audio: Callable[[], Awaitable[None]]
    shutdown_remote_audio: Callable[[], Awaitable[None]]
    stop_session_cleanup: Callable[[], None]
    try_export_on_stop: Callable[[], str | None]
    broadcast_runtime: Callable[[], Awaitable[None]]
    clear_after_stop: Callable[[], None]

    async def start(self) -> str:
        """
        Returns started_at_utc string from the session controller.
        """
        self.pre_start()
        self.select_speech_source()

        # Translation is used by TranscriptController early; start it first.
        await self.start_translation()
        await self.start_obs_captions()
        await self.apply_obs_settings()
        await self.reset_subtitles()

        self.on_start_reset()
        await self.init_asr_runtime_if_needed()
        started_at = self.start_session()
        self.capture_asr_mode_for_start()
        await self.start_speech_source()
        return started_at

    async def stop(self) -> str | None:
        self.pre_stop()

        await self.stop_speech_source()
        await self.safe_stop_audio()

        # Reset subtitles before shutting down translation to flush payloads deterministically.
        await self.reset_subtitles()
        await self.stop_translation()
        await self.stop_obs_captions()

        export_error = self.try_export_on_stop()
        await self.unload_asr_runtime_state()
        await self.shutdown_remote_audio()
        self.stop_session_cleanup()
        await self.broadcast_runtime()
        self.clear_after_stop()
        return export_error

