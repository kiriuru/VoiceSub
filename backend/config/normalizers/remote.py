from __future__ import annotations

from typing import Any


def normalize_remote_config(payload: Any, *, defaults: dict[str, Any]) -> dict[str, Any]:
    current = payload if isinstance(payload, dict) else {}
    enabled = bool(current.get("enabled", defaults["enabled"]))
    role = str(current.get("role", defaults["role"]) or defaults["role"]).strip().lower()
    if role not in {"disabled", "controller", "worker"}:
        role = "disabled"
    if not enabled:
        role = "disabled"

    lan = current.get("lan", {})
    if not isinstance(lan, dict):
        lan = {}
    controller = current.get("controller", {})
    if not isinstance(controller, dict):
        controller = {}
    worker = current.get("worker", {})
    if not isinstance(worker, dict):
        worker = {}

    try:
        lan_port = int(lan.get("port", defaults["lan"]["port"]) or defaults["lan"]["port"])
    except (TypeError, ValueError):
        lan_port = int(defaults["lan"]["port"])
    try:
        controller_connect_timeout_ms = int(
            controller.get("connect_timeout_ms", defaults["controller"]["connect_timeout_ms"])
            or defaults["controller"]["connect_timeout_ms"]
        )
    except (TypeError, ValueError):
        controller_connect_timeout_ms = int(defaults["controller"]["connect_timeout_ms"])
    try:
        controller_reconnect_delay_ms = int(
            controller.get("reconnect_delay_ms", defaults["controller"]["reconnect_delay_ms"])
            or defaults["controller"]["reconnect_delay_ms"]
        )
    except (TypeError, ValueError):
        controller_reconnect_delay_ms = int(defaults["controller"]["reconnect_delay_ms"])
    try:
        worker_heartbeat_timeout_ms = int(
            worker.get("heartbeat_timeout_ms", defaults["worker"]["heartbeat_timeout_ms"])
            or defaults["worker"]["heartbeat_timeout_ms"]
        )
    except (TypeError, ValueError):
        worker_heartbeat_timeout_ms = int(defaults["worker"]["heartbeat_timeout_ms"])

    bind_host = str(lan.get("bind_host", defaults["lan"]["bind_host"]) or defaults["lan"]["bind_host"]).strip()
    if not bind_host:
        bind_host = defaults["lan"]["bind_host"]

    return {
        "enabled": enabled,
        "role": role,
        "session_id": str(current.get("session_id", defaults["session_id"]) or "").strip(),
        "pair_code": str(current.get("pair_code", defaults["pair_code"]) or "").strip(),
        "lan": {
            "bind_enabled": bool(lan.get("bind_enabled", defaults["lan"]["bind_enabled"])),
            "bind_host": bind_host,
            "port": max(1, min(65535, lan_port)),
        },
        "controller": {
            "worker_url": str(controller.get("worker_url", defaults["controller"]["worker_url"]) or "").strip(),
            "connect_timeout_ms": max(1000, min(120000, controller_connect_timeout_ms)),
            "reconnect_delay_ms": max(100, min(30000, controller_reconnect_delay_ms)),
        },
        "worker": {
            "allow_unpaired": bool(worker.get("allow_unpaired", defaults["worker"]["allow_unpaired"])),
            "heartbeat_timeout_ms": max(1000, min(120000, worker_heartbeat_timeout_ms)),
        },
    }
