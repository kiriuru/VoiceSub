from __future__ import annotations

import unittest

from backend.core.runtime.browser_worker_state_controller import BrowserWorkerStateController


class BrowserWorkerStateControllerTests(unittest.TestCase):
    def test_default_state(self) -> None:
        c = BrowserWorkerStateController()
        self.assertFalse(c.external_worker_connected)
        self.assertIsNone(c.active_session_id)
        self.assertEqual(c.active_generation_id, 0)
        self.assertIsNone(c.last_status_signature)

    def test_mark_connected(self) -> None:
        c = BrowserWorkerStateController()
        c.mark_connected(session_id="s1", generation_id=42)
        self.assertTrue(c.external_worker_connected)
        self.assertEqual(c.active_session_id, "s1")
        self.assertEqual(c.active_generation_id, 42)

    def test_mark_disconnected(self) -> None:
        c = BrowserWorkerStateController()
        c.mark_connected(session_id="x", generation_id=1)
        c.mark_disconnected()
        self.assertFalse(c.external_worker_connected)

    def test_update_session(self) -> None:
        c = BrowserWorkerStateController()
        c.update_session(session_id="a", generation_id=9)
        self.assertEqual(c.active_session_id, "a")
        self.assertEqual(c.active_generation_id, 9)
        c.update_session(session_id=None, generation_id=12)
        self.assertIsNone(c.active_session_id)
        self.assertEqual(c.active_generation_id, 12)

    def test_reset_for_start(self) -> None:
        c = BrowserWorkerStateController()
        c.mark_connected(session_id="s", generation_id=3)
        c.set_status_signature((1, 2, 3))
        c.reset_for_start()
        self.assertFalse(c.external_worker_connected)
        self.assertIsNone(c.active_session_id)
        self.assertEqual(c.active_generation_id, 0)
        self.assertIsNone(c.last_status_signature)

    def test_reset_for_stop(self) -> None:
        c = BrowserWorkerStateController()
        c.mark_connected()
        c.set_status_signature(("a",))
        c.reset_for_stop()
        self.assertFalse(c.external_worker_connected)
        self.assertIsNone(c.last_status_signature)

    def test_status_signature_helpers(self) -> None:
        c = BrowserWorkerStateController()
        sig: tuple[object, ...] = ("x", 1)
        c.set_status_signature(sig)
        self.assertEqual(c.last_status_signature, sig)
        c.clear_status_signature()
        self.assertIsNone(c.last_status_signature)
        c.update_status_signature((True, False))
        self.assertEqual(c.last_status_signature, (True, False))

    def test_diagnostics(self) -> None:
        c = BrowserWorkerStateController()
        d = c.diagnostics()
        self.assertIn("external_worker_connected", d)
        self.assertIn("active_session_id", d)
        self.assertIn("active_generation_id", d)
        self.assertIn("has_status_signature", d)
        self.assertFalse(d["has_status_signature"])
        c.set_status_signature((1,))
        self.assertTrue(c.diagnostics()["has_status_signature"])


if __name__ == "__main__":
    unittest.main()
