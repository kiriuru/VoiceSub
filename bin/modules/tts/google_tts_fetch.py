#!/usr/bin/env python3
"""Fetch Google Translate TTS MP3 to stdout. Stdlib only (no gtts)."""

from __future__ import annotations

import base64
import os
import sys
import urllib.error
import urllib.parse
import urllib.request

USER_AGENT = (
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
    "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
)
MAX_CHARS = 200
TEXT_ENV_B64 = "VOICESUB_TTS_TEXT_B64"


def normalize_lang(lang: str) -> str:
    trimmed = (lang or "").strip().lower()
    if not trimmed:
        return "en"
    return trimmed.split("-")[0].split("_")[0] or "en"


def read_input_text() -> str:
    """Read UTF-8 text from env (preferred) or stdin bytes (never text-mode stdin)."""
    encoded = os.environ.get(TEXT_ENV_B64, "").strip()
    if encoded:
        return base64.b64decode(encoded).decode("utf-8")

    raw = sys.stdin.buffer.read()
    if not raw:
        return ""
    return raw.decode("utf-8")


def build_url(text: str, lang: str) -> str:
    tl = normalize_lang(lang)
    trimmed = text.strip()
    params = {
        "ie": "UTF-8",
        "q": trimmed,
        "tl": tl,
        "client": "tw-ob",
        "total": 1,
        "idx": 0,
        "textlen": len(trimmed),
    }
    return "https://translate.google.com/translate_tts?" + urllib.parse.urlencode(params)


def fetch_mp3(text: str, lang: str) -> bytes:
    if not text.strip():
        raise ValueError("empty text")
    if len(text) > MAX_CHARS:
        raise ValueError(f"text longer than {MAX_CHARS} chars")

    url = build_url(text, lang)
    request = urllib.request.Request(
        url,
        headers={
            "User-Agent": USER_AGENT,
            "Referer": "https://translate.google.com/",
        },
    )
    with urllib.request.urlopen(request, timeout=20) as response:
        data = response.read()
    if not data:
        raise RuntimeError("empty audio body from Google TTS")
    return data


def main() -> int:
    if len(sys.argv) < 2:
        sys.stderr.write("usage: google_tts_fetch.py <lang>  # text via env or stdin\n")
        return 2

    lang = sys.argv[1]
    text = read_input_text()
    try:
        data = fetch_mp3(text, lang)
    except urllib.error.HTTPError as err:
        sys.stderr.write(f"HTTP {err.code}: {err.reason}\n")
        return 1
    except Exception as err:  # noqa: BLE001 — CLI boundary
        sys.stderr.write(f"{err}\n")
        return 1

    sys.stdout.buffer.write(data)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
