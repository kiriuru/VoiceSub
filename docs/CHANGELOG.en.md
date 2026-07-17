# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<p align="center">
  <a href="./CHANGELOG.en.md">English</a> •
  <a href="./CHANGELOG.md">Русский</a>
</p>

This file covers the desktop line: **VoiceSub** (from `0.5.0`) and earlier **SST Desktop** releases (through `0.4.4`).

## [Unreleased]

## [0.6.0] - 2026-07-18

### Added

- **Local ASR** module (`/local-asr`) — offline Parakeet TDT via ONNX Runtime (CPU / optional CUDA), setup wizard, Modules card with ready / CPU / CUDA badges.
- Live ASR mode `local_parakeet` when the module is `ready` (in-process mic + VAD + decode; no Chrome Web Speech worker).
- Protected HTTP API `/api/asr/local/*` for status, config, deps, model download/load, EP probe, mic list, and test bench.
- Module settings under `user-data/modules/local-asr/`; project `asr.mode` stays in `user-data/config.toml`.
- `voicesub-partial-emit` crate (`word_growth` partial policy) wired into the existing subtitle / translation / overlay path.
- Latency presets `low` / `balanced` / `quality`, hallucination filter, emit telemetry, and setup checklist (deps → model → mic test → final).
- Installer artifact `VoiceSub_0.6.0_x64-setup.exe`.

### Changed

- Documentation and wiki mark Local ASR as shipped; Technical Architecture §18 documents the module.
- SST JSON import preserves `local_parakeet`; legacy `local` / experimental modes still map to `browser_google`.

## [0.5.5] - 2026-06-26

### Added

- Dedicated Tauri IPC pump with trailing-edge coalescing of dashboard `overlay_update` (default 90 ms; `VOICESUB_OVERLAY_IPC_MIN_INTERVAL_MS`; `0` disables). OBS `/ws/events` still receives every frame.
- Runtime metrics for bus lag and overlay IPC coalescing on `/api/runtime/status`.

### Changed

- Subtitle lifecycle runs before WS/IPC fanout; partial `transcript_update` broadcast is async so ingest is not blocked.
- Lock-free WS `global_sequence`; safer debounced snapshot resync after bus lag.
- Dashboard skips the frequent HTTP runtime poll when Tauri IPC is connected (30 s safety-net remains).

### Fixed

- Web Speech `audio-capture` errors auto-retry with exponential backoff instead of stopping recognition permanently.

## [0.5.4] - 2026-06-21

### Added

- Per-window Tauri `runtime-event` routing (dashboard vs TTS window).
- Leading-edge coalescing for partial `transcript_update` (default 90 ms; `VOICESUB_TRANSCRIPT_PARTIAL_MIN_INTERVAL_MS`).
- Browser worker: orphan PID reap on start; long-segment flush after monologues; overlap ASR handoff hardening.
- Advanced Web Speech settings in the dashboard; ingest latency diagnostics when full logging is on.

### Changed

- Live subtitle WebSocket fanout is `overlay_update` only; ASR is `transcript_update` only.
- `subtitle_payload_update` is Tauri IPC snapshot/replay only (not published on `/ws/events`).
- Browser worker process priority uses `ABOVE_NORMAL_PRIORITY_CLASS`.
- Diagnostic timestamps use RFC 3339 strings instead of epoch seconds.
- TTS volume range up to 150%; Twitch chat filters (mentions, digits, links) and IRC reconnect hardening.
- Deprecated subtitle lifecycle timing keys cleaned up in config.

### Removed

- Duplicate live broadcasts: `transcript_segment_event` and `subtitle_payload_update` on `/ws/events`.
- TTS module JS queue pump and deprecated TTS IPC surface.

### Fixed

- HTTP/WS fanout path correctness; Twitch chat log in the TTS module.
- TTS pipeline reliability (prefetch, config I/O, audio-chunk ordering).

## [0.5.3] - 2026-06-17

### Added

- Loopback API auth completion for protected `/api/*` (`x-voicesub-token`).
- GitHub Releases update check (`POST /api/updates/check`) with dashboard banner.
- Material 3 primary navigation shell on the dashboard.
- Background-tasks diagnostics on the HTTP status surface.

### Changed

- Browser Speech worker UI polish; TTS module loopback/styling; Twitch IRC auto-reconnect.
- Toolchain edition **Rust 2024**; CI and commit-convention docs.
- Migration: loopback token required for protected dashboard HTTP helpers (trusted pages inject `window.__VOICESUB_API_TOKEN__`).

### Fixed

- OBS overlay logging hardening (follow-up).
- Dead-code and unused i18n key prune.

## [0.5.2] - 2026-06-14

### Added

- Loopback API auth + overlay liveness checks.
- Rust TTS speech pipeline on the hot path; RuntimeEventBus snapshot improvements.
- Browser worker launch stability and overlap / browser-trace telemetry.

### Changed

- OBS Closed Captions send algorithm.

### Fixed

- TTS / Twitch issues on top of 0.5.1; dashboard UI polish.

## [0.5.1] - 2026-06-13

### Added

- Native dual-sink TTS (speech + Twitch) via Rust/cpal; **Sonic** tempo mode (pitch-preserving).
- Twitch multi-channel (up to 5 IRC joins per OAuth) with hot-apply chat filters.
- Resource telemetry bar; WebView2 power/memory policy; size-based log rotation.
- Translation / Web Speech top-20 language lists; OBS CC stable error codes with UI i18n.

### Changed

- TTS `playback_mode: "browser"` migrates to `sonic` on load; HTMLAudio playback path removed.
- Twitch legacy `channel` → `channels[0]`; digit preservation in chat TTS.
- Compact client logging by default (TTS UI traces require full logging).

### Fixed

- TTS enqueue IPC when `dropped_ids` is empty; Twitch language detection for link-only lines.
- Mic monitor leaks and cleaner Web Speech abort on worker stop.

### Removed

- Browser `HTMLAudio` / `setSinkId` playback path for TTS.

## [0.5.0] - 2026-06-10

First VoiceSub release (successor to SST Desktop `0.4.4`). Stack and delivery are new; subtitle/translation meaning preserved.

### Added

- Rust + Tauri 2 desktop app (`VoiceSub.exe`, NSIS `VoiceSub_{version}_x64-setup.exe`).
- Svelte 5 dashboard, vanilla OBS overlay, Svelte Web Speech worker (`/google-asr`).
- TTS module (`/tts`) with Twitch chat TTS; OBS Closed Captions (`voicesub-obs`).
- TOML config (`config_version` 8), SST `config.json` import, profiles, diagnostics ZIP.
- UI locales: en, ru, ja, ko, zh; GitHub Releases update check.

### Changed

- Product renamed to **VoiceSub**; default bind `127.0.0.1:8765`.
- Production ASR mode in core: `browser_google` (Chrome/Edge worker).

### Removed

- FastAPI / pywebview / PyInstaller SST desktop stack from active core.
- Legacy local ASR, remote controller/worker, experimental browser routes (archived under `legacy/`).
- Splash startup profiles.

## [0.4.4] - 2026-05-31

> Frozen SST Desktop line. Active development continues as VoiceSub.

### Security

- SSRF policy for OpenAI helper model routes when LAN bind is enabled.

### Added

- Shared overlay WebSocket stale-guard; desktop context store bridge.
- UI locales ja / ko / zh for dashboard, worker, and overlay.

### Changed

- Desktop launcher split into modules; overlay reconnect preserves last frame.

### Fixed

- Dashboard bootstrap error banner; bind/profile path safety tests.

## [0.4.3] - 2026-05-27

### Added

- Desktop profile lock and related launcher hardening (SST).

### Fixed

- Overlay and runtime stability follow-ups on the 0.4.2 line.

## [0.4.2] - 2026-05-25

### Added

- Further SST desktop polish toward the 0.4.x frozen line.

### Fixed

- Browser worker and overlay reconnect edge cases.

## [0.4.1] - 2026-05-20

### Added

- SST Desktop incremental features and config migrations on the 0.4.0 base.

### Fixed

- Dashboard and worker reliability patches.

## [0.4.0] - 2026-05-16

### Added

- SST Desktop 0.4.0 feature set (config_version lineage toward 7).

### Changed

- Architecture and packaging steps toward the frozen 0.4.4 baseline.

## [0.3.2] - 2026-05-14

### Fixed

- SST Desktop stability patches after 0.3.1.

## [0.3.1] - 2026-05-12

### Fixed

- SST Desktop follow-up fixes after the 0.3.0 modularization release.

## [0.3.0] - 2026-05-08

### Added

- Modular FastAPI backend services and frontend module stack (SST).
- Browser Speech session supervisor FSM; config migrations / schema export.

### Changed

- Thinner API routes; shared paths, logging, redaction utilities.

### Fixed

- WebSocket disconnect cleanup; runtime event coalescing; client log best-effort mode.

### Removed

- Unsupported backend ASR experiments from the active product surface.

## [0.2.9.2] - 2026-04-30

Earlier `0.2.9.*` SST Desktop history lives in archived GitHub release notes and is not expanded here.

[unreleased]: https://github.com/kiriuru/VoiceSub/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/kiriuru/VoiceSub/compare/v0.5.5...v0.6.0
[0.5.5]: https://github.com/kiriuru/VoiceSub/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/kiriuru/VoiceSub/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/kiriuru/VoiceSub/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/kiriuru/VoiceSub/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/kiriuru/VoiceSub/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/kiriuru/VoiceSub/releases/tag/v0.5.0
[0.4.4]: https://github.com/kiriuru/stream_sub_translator/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/kiriuru/stream_sub_translator/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/kiriuru/stream_sub_translator/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/kiriuru/stream_sub_translator/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/kiriuru/stream_sub_translator/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/kiriuru/stream_sub_translator/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/kiriuru/stream_sub_translator/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/kiriuru/stream_sub_translator/compare/v0.2.9.2...v0.3.0
[0.2.9.2]: https://github.com/kiriuru/stream_sub_translator/releases/tag/v0.2.9.2
