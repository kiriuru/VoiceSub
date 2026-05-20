from __future__ import annotations

import unittest

from backend.core.audio_capture import RNNoiseRecognitionProcessor
from backend.core.runtime.local_asr_recognition_processing import (
    apply_recognition_processing_settings,
    prepare_recognition_audio_bytes,
)


class _StubProcessor(RNNoiseRecognitionProcessor):
    def __init__(self) -> None:
        super().__init__(sample_rate=16000, channels=1)
        self.calls: list[bytes] = []

    def process_for_recognition(self, audio: bytes) -> bytes:  # type: ignore[override]
        self.calls.append(audio)
        return b"denoised:" + audio


class ApplyRecognitionProcessingSettingsTests(unittest.TestCase):
    def test_modern_key_enables_processor(self) -> None:
        processor = RNNoiseRecognitionProcessor(sample_rate=16000, channels=1)
        apply_recognition_processing_settings(
            config_getter=lambda: {"asr": {"rnnoise_enabled": True, "rnnoise_strength": 80}},
            rnnoise_processor=processor,
        )
        self.assertTrue(processor.enabled)
        self.assertEqual(processor.strength, 80)

    def test_legacy_key_still_enables_processor(self) -> None:
        processor = RNNoiseRecognitionProcessor(sample_rate=16000, channels=1)
        apply_recognition_processing_settings(
            config_getter=lambda: {"asr": {"experimental_noise_reduction_enabled": True}},
            rnnoise_processor=processor,
        )
        self.assertTrue(processor.enabled)

    def test_invalid_strength_falls_back_to_default(self) -> None:
        processor = RNNoiseRecognitionProcessor(sample_rate=16000, channels=1)
        apply_recognition_processing_settings(
            config_getter=lambda: {"asr": {"rnnoise_enabled": True, "rnnoise_strength": "bogus"}},
            rnnoise_processor=processor,
        )
        self.assertTrue(processor.enabled)
        self.assertEqual(processor.strength, 70)


class PrepareRecognitionAudioBytesTests(unittest.TestCase):
    def test_disabled_returns_raw_audio(self) -> None:
        processor = _StubProcessor()
        out = prepare_recognition_audio_bytes(
            b"\x01\x02",
            config_getter=lambda: {"asr": {"rnnoise_enabled": False}},
            rnnoise_processor=processor,
        )
        self.assertEqual(out, b"\x01\x02")
        self.assertEqual(processor.calls, [])

    def test_modern_key_triggers_processing(self) -> None:
        processor = _StubProcessor()
        out = prepare_recognition_audio_bytes(
            b"\x01\x02",
            config_getter=lambda: {"asr": {"rnnoise_enabled": True}},
            rnnoise_processor=processor,
        )
        self.assertEqual(out, b"denoised:\x01\x02")
        self.assertEqual(processor.calls, [b"\x01\x02"])

    def test_legacy_key_also_triggers_processing(self) -> None:
        processor = _StubProcessor()
        out = prepare_recognition_audio_bytes(
            b"\x01\x02",
            config_getter=lambda: {"asr": {"experimental_noise_reduction_enabled": True}},
            rnnoise_processor=processor,
        )
        self.assertEqual(out, b"denoised:\x01\x02")
        self.assertEqual(processor.calls, [b"\x01\x02"])

    def test_non_dict_asr_does_not_raise(self) -> None:
        processor = _StubProcessor()
        out = prepare_recognition_audio_bytes(
            b"\x01\x02",
            config_getter=lambda: {"asr": "not-a-dict"},
            rnnoise_processor=processor,
        )
        self.assertEqual(out, b"\x01\x02")
        self.assertEqual(processor.calls, [])


if __name__ == "__main__":
    unittest.main()
