from __future__ import annotations

import unittest
from pathlib import Path


class ParakeetLatencyPresetUiAlignmentTests(unittest.TestCase):
    def test_frontend_exports_fixed_slider_levels_for_named_presets(self) -> None:
        root = Path(__file__).resolve().parents[1]
        text = (root / "frontend" / "js" / "normalizers" / "parakeet-latency-presets.js").read_text(encoding="utf-8")
        self.assertIn("PARAKEET_LATENCY_PRESET_SIMPLE_LEVELS", text)
        self.assertIn("ultra_low_latency: { appearance: 5, finish: 5, stability: 3 }", text)
        self.assertIn("balanced: { appearance: 3, finish: 3, stability: 3 }", text)
        self.assertIn("quality: { appearance: 1, finish: 1, stability: 1 }", text)


if __name__ == "__main__":
    unittest.main()
