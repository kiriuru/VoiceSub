from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest import mock

import httpx
from fastapi import FastAPI

from backend.config import AppSettings, LocalConfigManager
from backend.services.config_state_service import ConfigStateService
from backend.services.update_service import UpdateService


class _FailingAsyncClient:
    """AsyncClient stand-in that always fails on GET (simulates offline GitHub)."""

    def __init__(self, *args, **kwargs) -> None:
        pass

    async def __aenter__(self) -> "_FailingAsyncClient":
        return self

    async def __aexit__(self, *args) -> bool:
        return False

    async def get(self, *args, **kwargs):  # type: ignore[no-untyped-def]
        raise httpx.ConnectError("simulated offline")


class UpdateServiceCheckNowTests(unittest.IsolatedAsyncioTestCase):
    def _make_app(self, tmp: Path) -> FastAPI:
        app = FastAPI()
        settings = AppSettings(data_dir=tmp / "user-data")
        config_manager = LocalConfigManager(settings)
        app.state.config_manager = config_manager
        app.state.config_state_service = ConfigStateService(app)
        app.state.update_service = UpdateService(app)
        return app

    async def test_check_now_network_failure_does_not_persist_updates_fields(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            app = self._make_app(tmp)
            config_manager: LocalConfigManager = app.state.config_manager
            config_state: ConfigStateService = app.state.config_state_service
            update_service: UpdateService = app.state.update_service

            payload = config_manager.default_config()
            payload["updates"] = {
                "enabled": True,
                "github_repo": "owner/repo",
                "release_channel": "stable",
                "check_interval_hours": 1,
                "latest_known_version": "0.1.0",
                "last_checked_utc": "2020-01-01T00:00:00+00:00",
            }
            saved = config_manager.save(payload)
            config_state.set_settings_saved(saved)
            disk_before = config_manager.load()

            with mock.patch(
                "backend.services.update_service.httpx.AsyncClient",
                side_effect=lambda *a, **k: _FailingAsyncClient(),
            ):
                result = await update_service.check_now(force=True)

            self.assertIn("Update check failed", result.get("sync", {}).get("message", ""))
            disk_after = config_manager.load()
            self.assertEqual(disk_after.get("updates", {}).get("last_checked_utc"), disk_before.get("updates", {}).get("last_checked_utc"))
            self.assertEqual(disk_after.get("updates", {}).get("latest_known_version"), disk_before.get("updates", {}).get("latest_known_version"))


if __name__ == "__main__":
    unittest.main()
