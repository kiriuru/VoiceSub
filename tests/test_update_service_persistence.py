from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from fastapi import FastAPI

from backend.config import AppSettings, LocalConfigManager
from backend.services.config_state_service import ConfigStateService
from backend.services.update_service import UpdateService


class UpdateServicePersistenceTests(unittest.TestCase):
    def _make_app(self, tmp: Path) -> FastAPI:
        app = FastAPI()
        settings = AppSettings(data_dir=tmp / "user-data")
        config_manager = LocalConfigManager(settings)
        app.state.config_manager = config_manager
        app.state.config_state_service = ConfigStateService(app)
        app.state.update_service = UpdateService(app)
        return app

    def test_update_check_does_not_persist_runtime_start_snapshot_payload(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            app = self._make_app(tmp)
            config_manager: LocalConfigManager = app.state.config_manager
            config_state: ConfigStateService = app.state.config_state_service
            update_service: UpdateService = app.state.update_service

            saved = config_manager.save(config_manager.default_config())
            config_state.set_settings_saved(saved)

            # Emulate runtime_start_snapshot with an unsaved runtime-only change.
            runtime_snapshot = dict(saved)
            runtime_snapshot["ui"] = dict(runtime_snapshot.get("ui", {}))
            runtime_snapshot["ui"]["language"] = "en" if runtime_snapshot["ui"].get("language") != "en" else "ru"
            config_state.set_runtime_start_snapshot(runtime_snapshot)

            before_disk = config_manager.load()
            self.assertNotEqual(
                before_disk.get("ui", {}).get("language"),
                config_state.current_payload().get("ui", {}).get("language"),
            )
            self.assertEqual(config_state.current_state().source, "runtime_start_snapshot")
            self.assertFalse(config_state.current_state().persisted)

            checked_utc = "2026-05-08T12:34:56+00:00"
            update_service._persist_updates(latest_version="9.9.9", checked_utc=checked_utc)

            # Disk: only updates.* changed; runtime-only UI change must NOT be persisted.
            after_disk = config_manager.load()
            self.assertEqual(after_disk.get("updates", {}).get("latest_known_version"), "9.9.9")
            self.assertEqual(after_disk.get("updates", {}).get("last_checked_utc"), checked_utc)
            self.assertEqual(after_disk.get("ui", {}).get("language"), before_disk.get("ui", {}).get("language"))

            # Active runtime snapshot remains active after update check.
            self.assertEqual(config_state.current_state().source, "runtime_start_snapshot")
            self.assertFalse(config_state.current_state().persisted)

            # In-memory: updates metadata is patched for UI/diagnostics, but other snapshot fields remain.
            self.assertEqual(config_state.current_payload().get("updates", {}).get("latest_known_version"), "9.9.9")
            self.assertEqual(config_state.current_payload().get("updates", {}).get("last_checked_utc"), checked_utc)
            self.assertEqual(config_state.current_payload().get("ui", {}).get("language"), runtime_snapshot["ui"]["language"])

    def test_update_check_on_saved_config_refreshes_active_state(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            app = self._make_app(tmp)
            config_manager: LocalConfigManager = app.state.config_manager
            config_state: ConfigStateService = app.state.config_state_service
            update_service: UpdateService = app.state.update_service

            saved = config_manager.save(config_manager.default_config())
            config_state.set_settings_saved(saved)
            self.assertEqual(config_state.current_state().source, "settings_save")
            self.assertTrue(config_state.current_state().persisted)

            checked_utc = "2026-05-08T01:02:03+00:00"
            update_service._persist_updates(latest_version="0.3.99", checked_utc=checked_utc)

            self.assertEqual(config_state.current_state().source, "settings_save")
            self.assertTrue(config_state.current_state().persisted)
            self.assertEqual(config_state.current_payload().get("updates", {}).get("latest_known_version"), "0.3.99")
            self.assertEqual(config_state.current_payload().get("updates", {}).get("last_checked_utc"), checked_utc)


if __name__ == "__main__":
    unittest.main()

