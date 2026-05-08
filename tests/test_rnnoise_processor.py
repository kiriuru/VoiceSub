from __future__ import annotations

import unittest

from backend.core.audio_capture import RNNoiseRecognitionProcessor
from backend.core.runtime.audio_runtime_controller import prepare_recognition_audio


class RNNoiseProcessorTests(unittest.TestCase):
    def test_prepare_recognition_audio_enabled_does_not_crash_when_backend_unavailable(self) -> None:
        processor = RNNoiseRecognitionProcessor(sample_rate=16000, channels=1)
        processor.configure(enabled=True, strength=70)
        audio = (b"\x00\x01" * 320)  # 16-bit mono frames
        result = prepare_recognition_audio(audio, rnnoise_enabled=True, rnnoise_processor=processor)
        self.assertIsInstance(result, (bytes, bytearray))

    def test_prepare_recognition_audio_disabled_is_passthrough(self) -> None:
        processor = RNNoiseRecognitionProcessor(sample_rate=16000, channels=1)
        audio = (b"\x00\x01" * 320)
        result = prepare_recognition_audio(audio, rnnoise_enabled=False, rnnoise_processor=processor)
        self.assertEqual(result, audio)


if __name__ == "__main__":
    unittest.main()

