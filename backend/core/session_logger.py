from __future__ import annotations

import json
import time
import threading
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from backend.core.redaction import redact_mapping, redact_text


class SessionLogManager:
    _LOG_FILE = "session-latest.jsonl"
    _CHANNELS = ("dashboard", "overlay", "browser_worker")
    _MAX_LINES = 5000

    def __init__(self, logs_dir: Path) -> None:
        self._logs_dir = logs_dir
        self._lock = threading.Lock()
        self._diagnostics = {
            "client_log_events_received": 0,
            "client_log_events_written": 0,
            "client_log_events_dropped": 0,
            "client_log_last_error": None,
            "client_log_last_error_kind": None,
        }
        self.reset()

    def reset(self) -> None:
        with self._lock:
            self._logs_dir.mkdir(parents=True, exist_ok=True)
            self._safe_write_text_locked("")
            self._reset_diagnostics_locked()

    def flush(self) -> None:
        return None

    def diagnostics(self) -> dict[str, Any]:
        with self._lock:
            return dict(self._diagnostics)

    def log(self, channel: str, message: str, *, source: str | None = None, details: dict | None = None) -> dict[str, Any]:
        with self._lock:
            self._diagnostics["client_log_events_received"] = int(self._diagnostics["client_log_events_received"]) + 1
        normalized_channel = self._normalize_channel(channel)
        normalized_message = " ".join(redact_text(message).strip().split())
        if not normalized_message:
            with self._lock:
                self._diagnostics["client_log_events_dropped"] = int(self._diagnostics["client_log_events_dropped"]) + 1
            return self._result(logged=False, reason="empty_message")
        sanitized_details = redact_mapping(details or {}) if details else None
        record = self._format_record(normalized_channel, normalized_message, source=source, details=sanitized_details)
        with self._lock:
            if not self._append_record_locked(record):
                self._mark_drop_locked("log_write_failed")
                return self._result(logged=False, reason="log_write_failed")
            self._diagnostics["client_log_events_written"] = int(self._diagnostics["client_log_events_written"]) + 1
            return self._result(logged=True)

    def _normalize_channel(self, channel: str) -> str:
        normalized = str(channel or "").strip().lower()
        return normalized if normalized in self._CHANNELS else "dashboard"

    def _log_path(self) -> Path:
        return self._logs_dir / self._LOG_FILE

    def _append_record_locked(self, record: dict) -> bool:
        line = json.dumps(record, ensure_ascii=False, sort_keys=True)
        if not self._safe_append_line_locked(f"{line}\n"):
            return False
        self._truncate_to_max_lines_locked()
        return True

    def _format_record(self, channel: str, message: str, *, source: str | None = None, details: dict | None = None) -> dict:
        return {
            "timestamp_utc": datetime.now(timezone.utc).isoformat(),
            "channel": channel,
            "type": "event",
            "source": str(source or "").strip().lower() or None,
            "message": message,
            "details": details or None,
        }

    def _safe_write_text_locked(self, text: str) -> bool:
        self._logs_dir.mkdir(parents=True, exist_ok=True)
        for attempt in range(2):
            try:
                self._log_path().write_text(text, encoding="utf-8")
                return True
            except (PermissionError, OSError, IOError) as exc:
                self._remember_error_locked(exc)
                if attempt == 0:
                    time.sleep(0.02)
        return False

    def _safe_append_line_locked(self, line: str) -> bool:
        self._logs_dir.mkdir(parents=True, exist_ok=True)
        for attempt in range(2):
            try:
                with self._log_path().open("a", encoding="utf-8") as handle:
                    handle.write(line)
                return True
            except (PermissionError, OSError, IOError) as exc:
                self._remember_error_locked(exc)
                if attempt == 0:
                    time.sleep(0.02)
        return False

    def _truncate_to_max_lines_locked(self) -> None:
        try:
            path = self._log_path()
            if not path.exists():
                return
            lines = path.read_text(encoding="utf-8").splitlines()
            if len(lines) <= self._MAX_LINES:
                return
            retained = "\n".join(lines[-self._MAX_LINES :]) + "\n"
            path.write_text(retained, encoding="utf-8")
        except (PermissionError, OSError, IOError) as exc:
            self._remember_error_locked(exc)

    def _remember_error_locked(self, exc: BaseException) -> None:
        self._diagnostics["client_log_last_error"] = str(exc)
        self._diagnostics["client_log_last_error_kind"] = type(exc).__name__

    def _mark_drop_locked(self, reason: str) -> None:
        self._diagnostics["client_log_events_dropped"] = int(self._diagnostics["client_log_events_dropped"]) + 1
        if reason:
            self._diagnostics["client_log_last_error_kind"] = reason

    def _reset_diagnostics_locked(self) -> None:
        self._diagnostics.update(
            {
                "client_log_events_received": 0,
                "client_log_events_written": 0,
                "client_log_events_dropped": 0,
                "client_log_last_error": None,
                "client_log_last_error_kind": None,
            }
        )

    @staticmethod
    def _result(*, logged: bool, reason: str | None = None) -> dict[str, Any]:
        payload: dict[str, Any] = {"ok": True, "logged": bool(logged)}
        if reason:
            payload["reason"] = reason
        return payload
