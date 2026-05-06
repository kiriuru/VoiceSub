from __future__ import annotations

from typing import Any
from urllib.parse import parse_qs, urlparse


def normalize_google_translate_api_key(raw_value: Any) -> str:
    trimmed = str(raw_value or "").strip()
    normalized = trimmed

    if "key=" in trimmed:
        parsed = urlparse(trimmed)
        query_values = parse_qs(parsed.query or trimmed)
        candidate = (query_values.get("key") or [""])[0].strip()
        if candidate:
            normalized = candidate

    if normalized.startswith("AIza") and "&" in normalized:
        candidate = normalized.split("&", 1)[0].strip()
        if candidate:
            normalized = candidate

    return normalized


def normalize_provider_secret(raw_value: Any, *, query_keys: tuple[str, ...] = ("key", "api_key")) -> str:
    trimmed = str(raw_value or "").strip()
    normalized = trimmed

    lowered = normalized.lower()
    if lowered.startswith("bearer "):
        normalized = normalized[7:].strip()

    if any(f"{key}=" in normalized for key in query_keys):
        parsed = urlparse(normalized)
        query_values = parse_qs(parsed.query or normalized)
        for key in query_keys:
            candidate = (query_values.get(key) or [""])[0].strip()
            if candidate:
                normalized = candidate
                break

    if "#" in normalized:
        normalized = normalized.split("#", 1)[0].strip()

    if "&" in normalized:
        candidate = normalized.split("&", 1)[0].strip()
        if candidate:
            normalized = candidate

    return normalized


def normalize_provider_text_value(raw_value: Any) -> str:
    return str(raw_value or "").strip()
