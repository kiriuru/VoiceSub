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

### Fixed

- Local ASR: restore `max_segment_ms` to **5500** (UI / SST parity). The **120000** preset/default disabled force-final — partials could grow for minutes without a Final when WebRTC VAD stayed sticky; loading a config with exactly `120000` heals it back to `5500`.

## [0.6.0] - 2026-07-18

### Added

- Fonts in `bin/fonts/`: **IBM Plex Mono**, **IBM Plex Serif**, **Source Code Pro** for dual-script presets and Cyrillic.
- Subtitle effects `pulse` and `reveal`.
- UI Theme: interface font picker (`ui.font_family`) applied to dashboard, Web ASR, TTS, and Local ASR via `ui_config_sync`.
- About VoiceSub credits dialog from the nav-rail avatar.
- Built-in **Help**: quick-start checklist and topic cards (recognition, translation, subtitles, style, OBS, tools) instead of a single prose block.
- **Local ASR** module (`/local-asr`) — offline Parakeet TDT via ONNX Runtime (CPU / optional CUDA), setup wizard, Modules card with ready / CPU / CUDA badges.
- Live ASR mode `local_parakeet` when the module is `ready` (in-process mic + VAD + decode; no Chrome Web Speech worker).
- Protected HTTP API `/api/asr/local/*` for status, config, deps, model download/load, EP probe, mic list, and test bench.
- Module settings under `user-data/modules/local-asr/`; project `asr.mode` stays in `user-data/config.toml`.
- `voicesub-partial-emit` crate (`word_growth` partial policy) wired into the existing subtitle / translation / overlay path.
- Latency presets `low` / `balanced` / `quality`, hallucination filter, emit telemetry, and setup checklist (deps → model → mic test → final).
- China-market translation providers with free-tier quotas: `baidu_translate`, `youdao_translate`, `tencent_tmt`, and `caiyun_translator` (zh/en/ja); **17** providers total, grouped as **China / Free-tier** (i18n hints/status).
- “Open provider setup / API keys” button for cloud providers (Google, Azure, DeepL, OpenAI, OpenRouter, LibreTranslate, Baidu, Youdao, Tencent, Caiyun, etc.; local LLMs excluded).
- LLM: **Override default subtitle prompt** checkbox (hideable custom prompt) for OpenAI / OpenRouter / LM Studio / Ollama.
- OpenAI: **Show models** loads a live list via `POST /api/openai/models` (official `/v1/models`, chat-model filter); curated list updated from the 2026 OpenAI catalog (`gpt-5.6-*`, `gpt-5.4-*`, …); **Show all chat models** toggle.
- LM Studio: **Test connection** probes `base_url` and loads available models.
- Installer artifact `VoiceSub_0.6.0_x64-setup.exe`.

### Fixed

- Local ASR / runtime idle CPU: heartbeat no longer runs a full `env_check` (CUDA/DLL scan) every tick — `diagnostics()` and `GET /api/asr/local/status` use the status cache; DLL lookup indexes each directory once; mic enumeration runs in `spawn_blocking`; Local ASR window defers mic list until after first paint; dialog open/close avoids re-entrancy spin.
- Local ASR: WebView2 memory/CPU blow-up on module open — `setLocale` is idempotent (stops `sst:locale-changed` + BroadcastChannel feedback loops); module no longer connects to `/ws/events` for UI sync (Tauri IPC is enough).
- TTS: same — disable `/ws/events` for UI sync (IPC already delivers `ui_config_sync`); locale sync uses `applyDashboardLocale` / idempotent `setLocale` (no Local-ASR-style leak thanks to the existing guard, but WS was still receiving overlay/runtime frames).
- Main dashboard: not affected by that leak (UI sync publisher only — no subscribe / `sst:locale-changed` loop; `ui_config_sync` ignored in the store); `applyUiFromConfig` skips re-applying theme/locale/sync when the presentation signature is unchanged.
- Startup/shutdown: Local ASR `env_check` no longer blocks service construction (CUDA Toolkit bin scans); DLL lookup checks direct paths first again; dashboard applies settings/theme before waiting on runtime status; last-known theme restored from localStorage before HTTP.
- Tools & Data: clear success/error feedback correctly; confirm before load/overwrite profiles; block deleting `default`; validate profile names; warn on importing redacted config; disable import while busy; show Local ASR readiness; drop duplicate stale-dropped metric; full logging applied live after Save (no false restart hint); save/delete no longer report success if profile list refresh fails; `diagnostics_update` keeps `local_module` / `active_mode`; success/error modal explains where files were saved (Downloads / `user-data/exports` / `user-data/profiles`).
- Profiles: seed/upgrade sparse `default.json` from full factory defaults; reject Windows reserved names.
- Diagnostics export: unique `diagnostics-{secs}_{ms}.zip` names; keep newest 12 ZIPs (prune is best-effort so export still succeeds); clearer HTTP error messages for delete/export.
- Docs: profiles are `{name}.json` (not `.toml`); diagnostics ZIP retention documented.
- Dashboard: checkbox/select edits no longer reset window size and position (resize+center only when `ui.layout` changes).
- UI language change persists the current in-memory config (no longer overwrites import/profile via stale `lastSavedConfig`).
- Web Speech advanced settings no longer force `asr.mode = browser_google`.
- OBS overlay: stale-guard activates after the module script loads; runtime-gone timers no longer pile up.
- Browser worker: autostart timer and lifecycle listeners cleared in `destroy()`.
- Section scroll-spy uses the shell scroll container; Subtitles nav opens the panel with top tabs Subtitles/Style directly (no intermediate hub).
- Primary tab icons (nav rail / bottom nav) enlarged ~15%.
- TTS / Local ASR / Web Speech worker scroll: `overflow: hidden` scoped to the dashboard standard shell only (not global `html/body`).
- Local ASR: setup Close and “Re-check” buttons match the rest of the UI.
- Dark-theme dialogs readable again (runtime Details, credits, Local ASR status/alert/setup) — `color-scheme` plus explicit text color instead of UA `CanvasText`.
- UI theme hot-applies to Local ASR and TTS without Save (IPC + WS fallback); i18n for `style.ui_theme.font` / `font.default`.
- Browser Google ASR lifecycle:
  - failed Chrome launch clears `runtime_running` and stops browser speech ingest (same as Local ASR failure path);
  - PID tracking / `browser-worker.pid` cleared only when Chrome is actually gone; a still-live process after failed `taskkill` is kept for orphan reap;
  - IPC `launch_browser_worker` records PID in the shared orchestrator and terminates any previous worker (no second orphan Chrome);
  - Local ASR start reaps leftover Chrome first;
  - `generationId` bumps on every controlled start; pending restart cancel uses `stopEpoch` (user/control stop);
  - WS transport replace drops the previous outbound **without** sending `stop` (avoids killing recognition on reconnect).
- **Word replace (pre-translation):** cached Aho-Corasick/regex; CJK with default whole-words; stems; mask form `fuck`→`f*ck` (not `***`); already-masked `f*ck` is left alone. Subtitle lifecycle unchanged.
- IPC ACL: `get_loopback_api_token` allowlisted again (fallback when HTML injection is missing).
- `runtime-event`: `listen` → buffer → snapshot → drain (live frames are not overwritten by a stale snapshot); dashboard snapshot prefers `overlay_update`; TTS snapshot is `runtime_update` + `twitch_connection_update` only.
- `tts-speech-activity` / `playback-finished` use `emit_to(tts)` only (not a global emit into main/local-asr).
- Twitch chat → `RuntimeEventBus` only (no OBS `/ws/events` flood); connection updates still hit the hub for replay.
- Lag-resync: pending queue (last needed sync is never dropped); discard coalesced overlay on `Lagged` so a timer cannot regress UI after snapshot.
- `Jet Brains Mono` name matches the font catalog; `JetBrains Mono` alias on normalize.
- OBS overlay keeps `style_slot` / `slot_id`; dashboard preview calls `disposeRenderContainer` on `render().empty`.
- Renderer: `colorToRgba` (named/`rgb()`/`#rrggbbaa`); emoji on code-point boundaries; whitespace-only filtered; `inferStyleSlot` + `slot_id`; fast path skips disconnected surfaces.
- LM Studio / Ollama: JIT model load is no longer aborted by the default 10s timeout (`Engine protocol startup was aborted` / `Model is unloaded`); local providers get a **≥120s** timeout floor; LM Studio requests include `ttl`.
- Provider setup buttons open the system browser (`open_external_https_url` allowlist includes provider console hosts).
- Baidu Translate: POST form-urlencoded instead of GET; `sv` → `swe` language map; Youdao parses numeric `errorCode`.
- Translation persistent cache no longer wiped on every runtime start; disk cache survives restart when settings are unchanged.
- Translation dispatcher no longer leaks `active_jobs` when the same sequence is submitted twice; queue overflow no longer holds the dispatcher lock across relevance checks.
- Live settings apply for translation awaits the engine lock (API keys / lines are not silently skipped) and refreshes provider concurrency limits.
- HTTP translation timeouts honor configured `timeout_ms` (per-request timeout wired through all providers).
- Local LLM readiness probe accepts hostnames such as `localhost` (DNS resolve), not only IP literals.
- Preview supersession is robust to generation counter edge cases; short-circuit empty/identical-lang results no longer report a false cache hit.

### Changed

- Style panel: compact numeric field grid; text align and effect on one row.
- Built-in style catalog rebuilt (**20** presets): themed dual-script stacks (Film Noir, Retro Terminal, Fallout, Anime Stream, and others); near-identical dark plates collapsed to **4 materials** — Max Contrast, Podcast Subtle (parchment), Glass Frost (milky ice ~44%), Twitch Lower-Third (`#9146FF` + Oswald). Removed `sakura_soft`, `minimal_mono`, `editorial_news` (migrate → `meeting_soft` / `glass_frost` / `dark_cinema`).
- **Retro Terminal**: Cyrillic via **IBM Plex Serif Regular**.
- Latin-only faces in `/project-fonts.css` declare `unicode-range` so Cyrillic falls through to the next stack face (Plex / Ubuntu / Noto / Comfortaa…).
- Outline width: **ASS/Aegisub 0–4 px** scale (step 0.1).
- Effects: `fade` is opacity-only; `glow` follows fill color; OBS partials cheapen heavy `blur_in`/`glow` to `fade`; `prefers-reduced-motion` honored.
- Tauri IPC capabilities split per window: **main** (full shell), **tts** (playback/Twitch/snapshot), **local-asr** (token + allowlisted URLs).
- TTS: HTTP `/api/runtime/status` poll only when `runtime-event` is down; speech-context poll slower while IPC is healthy (focus still refreshes immediately).
- Browser Speech worker is **Google Chrome only** (`/google-asr`): removed `/google-asr-edge`; import `browser_google_edge` → `browser_google`; orphan reap only for `chrome.exe`.
- Browser worker CPU affinity is **opt-in** (`VOICESUB_BROWSER_AFFINITY` / `VOICESUB_BROWSER_AFFINITY_MASK`); off by default.
- `runtime-event` routing: **local-asr** window receives `ui_config_sync` (live theme/locale/font); `/ws/events` replays the last `ui_config_sync` on connect.
- Help copy updated for Local ASR / Modules / word replace; HelpPanel i18n reacts to locale changes.
- Documentation and wiki mark Local ASR as shipped; Technical Architecture §18 documents the module.
- SST JSON import preserves `local_parakeet`; legacy `local` / experimental modes still map to `browser_google`.
- Translation `timeout_ms` / HTTP client ceiling: **300s** (was 60s); Settings UI and config normalize aligned.
- Persistent translation cache path is `user-data/translation-cache/` (legacy `user-data/cache/translation_cache.json` is copied once on upgrade).
- Translation cache keys hash source text; cache flushes on engine drop; LLM `used_default_prompt` / `override_prompt` settings.
- DeepL maps UI language codes (`en`/`zh-cn`/`pt`, …) to API targets and auto-selects Free vs Pro URL from the API key (`:fx` → free).
- Google Cloud Translation v3 expands short model ids to full resource names; Google v3 settings labels i18n added.
- Azure / LibreTranslate map Chinese UI codes (`zh-Hans`/`zh-Hant`, `zh`/`zt`); readiness surfaces soft warnings for empty Azure region and public LibreTranslate.

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
