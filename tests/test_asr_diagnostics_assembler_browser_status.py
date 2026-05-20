from __future__ import annotations

import unittest

from backend.core.asr_provider_selection import BROWSER_GOOGLE_EXPERIMENTAL_MODE, BROWSER_GOOGLE_MODE
from backend.core.runtime.asr_diagnostics_assembler import build_browser_asr_provider_status


class BuildBrowserAsrProviderStatusTests(unittest.TestCase):
    def test_connected_classic_mode(self) -> None:
        status = build_browser_asr_provider_status(
            browser_mode=BROWSER_GOOGLE_MODE,
            external_worker_connected=True,
            is_runtime_running=True,
        )
        self.assertEqual(status.provider, BROWSER_GOOGLE_MODE)
        self.assertTrue(status.ready)
        self.assertIn("connected", status.message.lower())
        self.assertTrue(status.runtime_initialized)

    def test_disconnected_experimental_mode(self) -> None:
        status = build_browser_asr_provider_status(
            browser_mode=BROWSER_GOOGLE_EXPERIMENTAL_MODE,
            external_worker_connected=False,
            is_runtime_running=False,
        )
        self.assertEqual(status.provider, BROWSER_GOOGLE_EXPERIMENTAL_MODE)
        self.assertIn("experimental", status.message.lower())
        self.assertFalse(status.runtime_initialized)


if __name__ == "__main__":
    unittest.main()
