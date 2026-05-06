from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from backend.config import AppSettings, LocalConfigManager
from backend.core.config_migrations import CURRENT_CONFIG_VERSION, migrate_config
from backend.core.profile_manager import ProfileManager


PROJECT_ROOT = Path(__file__).resolve().parents[1]


def _removed_provider_value() -> str:
    return "_".join(["google", "legacy", "http", "experimental"])


def _removed_provider_key() -> str:
    return "_".join(["google", "legacy", "http"])


class ConfigMigrationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name)
        self.manager = LocalConfigManager(AppSettings(data_dir=self.root / "user-data"))

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def test_old_config_without_version_migrates_to_current_schema(self) -> None:
        migrated = self.manager.save(
            {
                "targets": ["en", "ja"],
                "translation": {
                    "enabled": True,
                    "provider": "google_translate_v2",
                    "provider_settings": {"google_translate_v2": {"api_key": "AIza-demo"}},
                },
                "subtitle_style": {"preset": "clean_default", "custom_presets": {"stream": {"label": "Stream"}}},
            }
        )

        self.assertEqual(migrated["config_version"], CURRENT_CONFIG_VERSION)
        self.assertIn("ui", migrated)
        self.assertIn("asr", migrated)
        self.assertIn("translation", migrated)
        self.assertFalse(migrated["remote"]["enabled"])
        self.assertEqual(migrated["translation"]["target_languages"], ["en", "ja"])
        self.assertIn("custom_presets", migrated["subtitle_style"])

    def test_migrate_config_renames_legacy_parakeet_provider(self) -> None:
        migrated = migrate_config(
            {
                "config_version": 2,
                "asr": {"provider_preference": "official_eu_parakeet_realtime"},
            }
        )

        self.assertEqual(migrated["config_version"], CURRENT_CONFIG_VERSION)
        self.assertEqual(migrated["asr"]["provider_preference"], "official_eu_parakeet_low_latency")

    def test_removed_legacy_provider_preference_migrates_to_low_latency_parakeet(self) -> None:
        migrated = migrate_config(
            {
                "config_version": CURRENT_CONFIG_VERSION,
                "asr": {
                    "mode": "local",
                    "provider_preference": _removed_provider_value(),
                },
            }
        )

        self.assertEqual(migrated["asr"]["mode"], "local")
        self.assertEqual(migrated["asr"]["provider_preference"], "official_eu_parakeet_low_latency")

    def test_removed_legacy_asr_section_is_dropped_during_migration(self) -> None:
        migrated = migrate_config(
            {
                "config_version": CURRENT_CONFIG_VERSION,
                "asr": {
                    "mode": "local",
                    "provider_preference": _removed_provider_value(),
                    _removed_provider_key(): {
                        "enabled": True,
                        "api_key": "deprecated-secret",
                        "host_override": "https://example.test",
                    },
                },
            }
        )

        self.assertEqual(migrated["config_version"], CURRENT_CONFIG_VERSION)
        self.assertEqual(migrated["asr"]["provider_preference"], "official_eu_parakeet_low_latency")
        self.assertNotIn(_removed_provider_key(), migrated["asr"])

    def test_manager_normalization_does_not_resurrect_removed_legacy_asr_section(self) -> None:
        saved = self.manager.save(
            {
                "asr": {
                    "mode": "local",
                    "provider_preference": _removed_provider_value(),
                    _removed_provider_key(): {
                        "enabled": True,
                        "api_key": "deprecated-secret",
                    },
                }
            }
        )

        self.assertEqual(saved["asr"]["provider_preference"], "official_eu_parakeet_low_latency")
        self.assertNotIn(_removed_provider_key(), saved["asr"])

    def test_config_schema_excludes_removed_legacy_provider(self) -> None:
        schema_json = (PROJECT_ROOT / "backend" / "data" / "config.schema.json").read_text(encoding="utf-8")
        example_json = (PROJECT_ROOT / "backend" / "data" / "config.example.json").read_text(encoding="utf-8")
        self.assertNotIn(_removed_provider_key(), schema_json)
        self.assertNotIn(_removed_provider_value(), schema_json)
        self.assertNotIn(_removed_provider_key(), example_json)
        self.assertNotIn(_removed_provider_value(), example_json)

    def test_profiles_also_migrate_to_current_schema(self) -> None:
        profiles_dir = self.root / "profiles"
        manager = ProfileManager(profiles_dir, payload_normalizer=self.manager.normalize_profile_payload)
        legacy_profile = {
            "translation": {
                "enabled": True,
                "target_languages": ["fr"],
            },
            "asr": {
                "provider_preference": _removed_provider_value(),
            },
            "subtitle_style": {
                "preset": "clean_default",
            },
        }
        (profiles_dir / "caster.json").parent.mkdir(parents=True, exist_ok=True)
        (profiles_dir / "caster.json").write_text(json.dumps(legacy_profile, ensure_ascii=False, indent=2), encoding="utf-8")

        loaded = manager.load_profile("caster")

        self.assertEqual(loaded["config_version"], CURRENT_CONFIG_VERSION)
        self.assertEqual(loaded["profile"], "caster")
        self.assertEqual(loaded["asr"]["provider_preference"], "official_eu_parakeet_low_latency")
        self.assertFalse(loaded["remote"]["enabled"])
        self.assertEqual(loaded["translation"]["target_languages"], ["fr"])
        self.assertEqual(loaded["subtitle_style"]["preset"], "clean_default")


if __name__ == "__main__":
    unittest.main()
