from __future__ import annotations

import unittest

from backend.core.subtitle_style import (
    merge_style_presets,
    normalize_subtitle_style_config,
    resolve_effective_subtitle_style,
)


class SubtitleStyleEffectsTests(unittest.TestCase):
    def test_normalize_preserves_extended_web_effects(self) -> None:
        for effect in ("slide_up", "zoom_in", "blur_in", "glow", "fade", "subtle_pop", "none"):
            with self.subTest(effect=effect):
                payload = {
                    "preset": "clean_default",
                    "base": {"effect": effect},
                }
                normalized = normalize_subtitle_style_config(payload)
                self.assertEqual(normalized["base"]["effect"], effect)

    def test_normalize_rejects_unknown_effect_to_default(self) -> None:
        payload = {
            "preset": "clean_default",
            "base": {"effect": "spin_wildly"},
        }
        normalized = normalize_subtitle_style_config(payload)
        self.assertEqual(normalized["base"]["effect"], "none")

    def test_line_slot_override_applies_font_family_and_size(self) -> None:
        payload = {
            "preset": "clean_default",
            "base": {"font_family": "Base Font", "font_size_px": 30},
            "line_slots": {
                "source": {
                    "enabled": True,
                    "font_family": "Source Font",
                    "font_size_px": 40,
                },
                "translation_1": {
                    "enabled": True,
                    "font_size_px": 24,
                },
            },
        }
        normalized = normalize_subtitle_style_config(payload)
        effective = resolve_effective_subtitle_style(normalized)
        self.assertEqual(effective["line_slots"]["source"]["font_family"], "Source Font")
        self.assertEqual(effective["line_slots"]["source"]["font_size_px"], 40)
        self.assertEqual(effective["line_slots"]["translation_1"]["font_family"], "Base Font")
        self.assertEqual(effective["line_slots"]["translation_1"]["font_size_px"], 24)

    def test_builtin_presets_include_accessibility_dark_cinema_meeting_soft(self) -> None:
        catalog = merge_style_presets()
        for key in ("accessibility_high_contrast", "dark_cinema", "meeting_soft"):
            with self.subTest(preset=key):
                self.assertIn(key, catalog)
                self.assertTrue(catalog[key].get("built_in"))


if __name__ == "__main__":
    unittest.main()
