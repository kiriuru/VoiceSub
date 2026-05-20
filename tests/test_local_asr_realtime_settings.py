from __future__ import annotations

import unittest
from unittest import mock

from backend.core.runtime.local_asr_realtime_settings import resolve_realtime_settings


class LocalAsrRealtimeSettingsTests(unittest.TestCase):
    def test_balanced_preset_applies_word_growth(self) -> None:
        engine = mock.Mock()
        engine.status.return_value = mock.Mock(provider="official_eu_parakeet_low_latency")
        settings = resolve_realtime_settings(
            config_getter=lambda: {
                "asr": {
                    "realtime": {
                        "latency_preset": "balanced",
                    }
                }
            },
            asr_engine=engine,
        )
        self.assertEqual(settings.get("partial_emit_mode"), "word_growth")
        self.assertEqual(settings.get("partial_min_new_words"), 1)
        self.assertEqual(settings.get("partial_emit_interval_ms"), 280)

    def test_non_low_latency_provider_uses_legacy_defaults(self) -> None:
        engine = mock.Mock()
        engine.status.return_value = mock.Mock(provider="official_eu_parakeet")
        settings = resolve_realtime_settings(
            config_getter=lambda: {"asr": {"realtime": {"latency_preset": "balanced"}}},
            asr_engine=engine,
        )
        self.assertEqual(settings.get("partial_emit_interval_ms"), 450)
