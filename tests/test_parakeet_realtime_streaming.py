from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest import mock

import numpy as np

from backend.core.parakeet_provider import AsrResult, OfficialEuParakeetRealtimeProvider


class _FakeStreamingState:
    def __init__(self) -> None:
        self.pending_audio = np.zeros((0,), dtype=np.float32)
        self.processed_samples = 0
        self.current_text = ""
        self.first_step = True


class ParakeetRealtimeStreamingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.models_dir = Path(self.temp_dir.name)
        self.provider = OfficialEuParakeetRealtimeProvider(self.models_dir, prefer_gpu=False)

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def test_append_cumulative_audio_only_feeds_delta(self) -> None:
        state = _FakeStreamingState()
        first = np.linspace(-0.1, 0.1, 1600, dtype=np.float32)
        second = np.linspace(-0.2, 0.2, 3200, dtype=np.float32)

        self.provider._append_cumulative_audio(state, first)  # noqa: SLF001
        self.assertEqual(int(state.processed_samples), 1600)
        self.assertEqual(int(state.pending_audio.shape[0]), 1600)

        self.provider._append_cumulative_audio(state, second)  # noqa: SLF001
        self.assertEqual(int(state.processed_samples), 3200)
        self.assertEqual(int(state.pending_audio.shape[0]), 3200)

    def test_streaming_partial_text_is_prefix_of_final(self) -> None:
        state = _FakeStreamingState()

        def _fake_decode_available(*, model: object, state: _FakeStreamingState) -> None:  # noqa: ARG001
            sample_count = int(state.processed_samples)
            if sample_count < 800:
                state.current_text = ""
            elif sample_count < 3200:
                state.current_text = "hello"
            else:
                state.current_text = "hello world"

        def _fake_flush(*, model: object, state: _FakeStreamingState, sample_rate: int) -> None:  # noqa: ARG001
            state.current_text = "hello world"

        audio_partial = np.zeros(2400, dtype=np.int16).tobytes()
        audio_final = np.zeros(4000, dtype=np.int16).tobytes()

        with (
            mock.patch.object(self.provider, "_ensure_loaded", return_value=object()),
            mock.patch.object(self.provider, "_streaming_decode_enabled", return_value=True),
            mock.patch.object(
                self.provider,
                "_get_or_create_stream_state",
                return_value=state,
            ),
            mock.patch.object(self.provider, "_decode_available_audio", side_effect=_fake_decode_available),
            mock.patch.object(self.provider, "_flush_final_audio", side_effect=_fake_flush),
            mock.patch.object(self.provider, "_torch_inference_context", return_value=mock.MagicMock()),
        ):
            partial_result = self.provider.transcribe(
                audio_partial,
                sample_rate=16000,
                is_final=False,
                segment_id="seg-1",
            )
            final_result = self.provider.transcribe(
                audio_final,
                sample_rate=16000,
                is_final=True,
                segment_id="seg-1",
            )

        self.assertEqual(partial_result.partial, "hello")
        self.assertEqual(final_result.final, "hello world")
        final_words = final_result.final.split()
        partial_words = partial_result.partial.split()
        self.assertEqual(partial_words, final_words[: len(partial_words)])

    def test_streaming_disabled_uses_batch_path(self) -> None:
        with (
            mock.patch.object(self.provider, "_ensure_loaded", return_value=object()),
            mock.patch.object(self.provider, "_streaming_decode_enabled", return_value=False),
            mock.patch.object(
                self.provider,
                "_transcribe_batch",
                return_value=AsrResult(),
            ) as batch_mock,
        ):
            self.provider.transcribe(b"\x00\x00", sample_rate=16000, is_final=False, segment_id="seg-2")

        batch_mock.assert_called_once()
