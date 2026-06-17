# VoiceSub 0.5.3 — Technical Architecture Document

Valid for the codebase where `voicesub-types::PROJECT_VERSION = "0.5.3"`.

This document describes the VoiceSub project layout, HTTP/WebSocket/Tauri IPC contracts, configuration schema, data flow through the Rust runtime, and frontend surfaces. It is the **canonical technical reference** for active development. README is a short product overview; CHANGELOG is release history; agent policy is `AGENTS.md`.

**Maintenance rule:** any change to API/WS/IPC contracts, config schema, subtitle/translation lifecycle, overlay renderer, browser worker, or NSIS installer bundle **updates the corresponding sections in the same task**. Outdated wording is removed or rewritten — not kept "for history".

## Table of Contents

- [Related Documentation](#related-documentation)
- [Quick Reference](#quick-reference)
- [1. Purpose and System Boundaries](#1-purpose-and-system-boundaries)
- [2. Technology Stack](#2-technology-stack)
- [3. High-Level Runtime Diagram](#3-high-level-runtime-diagram)
- [4. Repository Layout](#4-repository-layout)
- [5. Rust Workspace (crates)](#5-rust-workspace-crates)
- [6. RuntimeService: Orchestration and Lifecycle](#6-runtimeservice-orchestration-and-lifecycle)
- [7. Configuration and Migrations](#7-configuration-and-migrations)
- [8. HTTP API (local)](#8-http-api-local)
- [9. WebSocket Surface](#9-websocket-surface)
- [10. Tauri IPC](#10-tauri-ipc)
- [11. Logs, Diagnostics, Export](#11-logs-diagnostics-export)
- [12. Browser Speech Worker](#12-browser-speech-worker)
- [13. Translation: Lifecycle and Invariants](#13-translation-lifecycle-and-invariants)
- [14. Subtitle Lifecycle and Presentation](#14-subtitle-lifecycle-and-presentation)
- [15. Subtitle Styles and Overlay](#15-subtitle-styles-and-overlay)
- [16. OBS Closed Captions](#16-obs-closed-captions)
- [17. TTS Module](#17-tts-module)
- [18. Desktop Runtime and NSIS Release](#18-desktop-runtime-and-nsis-release)
- [19. Storage and Paths](#19-storage-and-paths)
- [20. Frontend: Dashboard (Svelte)](#20-frontend-dashboard-svelte)
- [21. Frontend: Overlay (vanilla)](#21-frontend-overlay-vanilla)
- [22. Frontend: Browser Worker (Svelte)](#22-frontend-browser-worker-svelte)
- [23. UI Localization (i18n)](#23-ui-localization-i18n)
- [24. Versioning and Update Checks](#24-versioning-and-update-checks)
- [25. Testing](#25-testing)
- [26. Product Invariants](#26-product-invariants)
- [27. Known Limitations & Technical Debt](#27-known-limitations--technical-debt)
- [28. Security & Privacy Model](#28-security--privacy-model)
- [29. Extension Points](#29-extension-points)
- [30. Glossary](#30-glossary)

## Related Documentation

| Document | Purpose |
| --- | --- |
| `docs/WIKI.en.md` | User guide (EN) |
| `docs/WIKI.ru.md` | User guide (RU) |
| `docs/TECHNICAL_ARCHITECTURE.en.md` | Technical architecture (English) |
| `docs/CHANGELOG.md` | Change history |
| `AGENTS.md` | Agent policy |

## Quick Reference

### Dev build and test

```bash
# Rust tests
cargo test --workspace

# Frontend build (dashboard + worker + TTS)
npm run build

# NSIS release (Windows)
build-release-msi.bat   # → build-release.ps1
```

Tauri dev: embedded HTTP on `http://127.0.0.1:8765`; main webview opens the dashboard at that URL.

### Key URLs (default bind)

| URL | Purpose |
| --- | --- |
| `http://127.0.0.1:8765/` | Svelte dashboard |
| `http://127.0.0.1:8765/overlay` | OBS Browser Source |
| `http://127.0.0.1:8765/google-asr?autostart=1` | Browser Speech worker |
| `http://127.0.0.1:8765/google-asr-edge` | Same worker (Edge smoke) |
| `http://127.0.0.1:8765/tts` | TTS module UI |

### Key API endpoints

| Endpoint | Purpose |
| --- | --- |
| `POST /api/runtime/start` | Start session + launch Chrome worker |
| `POST /api/runtime/stop` | Stop worker, translation, OBS |
| `GET /api/runtime/status` | Runtime snapshot + diagnostics |
| `GET /api/settings/load` | Load config + presets + fonts |
| `POST /api/settings/save` | Normalize + save `config.toml` |
| `GET /api/exports/diagnostics` | Redacted diagnostics ZIP |
| `GET /api/obs/url` | `{ overlay_url }` for OBS |

### WebSocket channels

| Channel | Purpose |
| --- | --- |
| `/ws/events` | Dashboard, overlay, runtime/subtitle events |
| `/ws/asr_worker` | Browser Speech worker transport |

### Key files

| File | Purpose |
| --- | --- |
| `crates/voicesub-types/src/version.rs` | `PROJECT_VERSION` |
| `crates/voicesub-runtime/src/service.rs` | Orchestration, start/stop |
| `crates/voicesub-runtime/src/http/router.rs` | All HTTP/WS routes |
| `crates/voicesub-subtitle/src/lifecycle.rs` | Subtitle FSM/TTL |
| `crates/voicesub-translation/src/dispatcher.rs` | Translation queue + stale drop |
| `src-tauri/src/lib.rs` | Tauri shell + IPC |
| `bin/overlay/shared/js/subtitle-style.js` | Shared overlay renderer |

## 1. Purpose and System Boundaries

**VoiceSub** is a local Windows-first desktop app for real-time subtitles:

- speech capture via **Browser Speech worker** (separate Chrome/Edge window with visible address bar, Web Speech API);
- optional translation to 0..5 target languages with independent provider per slot;
- unified subtitle payload routing to Svelte dashboard, vanilla OBS overlay, and OBS Closed Captions;
- optional **TTS module** (subtitle speech, Twitch chat TTS);
- diagnostics ZIP export and client-side trace logs.

**Core ASR:** `browser_google` only — Web Speech at `/google-asr` and `/google-asr-edge`. Local ASR and experimental worker routes are not part of the product.

Hard boundaries:

- local-first runtime, default bind `127.0.0.1:8765`;
- no cloud backend, accounts, or hosted database;
- **Node.js forbidden in shipped runtime**; Vite/Node only on dev/build machines;
- dashboard and worker are Svelte (compile-time bundle); overlay is **vanilla HTML/JS** (no Svelte);
- **WebView2 Runtime** — required for the Tauri shell (`VoiceSub.exe`, dashboard, `/tts`); NSIS installer can run the bootstrapper if missing.
- Chrome is a separate system dependency for the Web Speech worker; core installer does not bundle Python/torch/Node.

## 2. Technology Stack

| Layer | Technologies |
| --- | --- |
| Core runtime | Rust 1.85+, Tokio, Axum 0.8 |
| Desktop shell | Tauri 2 → `VoiceSub.exe` (NSIS `setup.exe`) |
| Dashboard UI | Svelte 5 + Vite → `bin/dashboard/` |
| Browser worker | Svelte 5 + Vite → `bin/worker/` |
| TTS UI | Svelte 5 + Vite → `bin/tts/` |
| OBS overlay | Vanilla HTML/CSS/JS → `bin/overlay/` |
| Config | TOML (`user-data/config.toml`), JSON-shaped document inside |
| HTTP client (providers) | `reqwest` + rustls |
| Logging | `tracing` + rotating files + opt-in JSONL |
| TTS sidecar | Embedded Python exe in `bin/modules/tts/runtime/` (not core Rust) |

**Forbidden in active tree:** React, Webpack, Electron, pywebview, FastAPI runtime, in-process NeMo/torch.

## 3. High-Level Runtime Diagram

```mermaid
flowchart LR
  subgraph Shell["Desktop shell"]
    TA["Tauri main window<br/>→ http://127.0.0.1:8765/"]
  end

  subgraph Core["Rust core (voicesub-runtime)"]
    HTTP["Axum HTTP/WS"]
    RT["RuntimeService"]
    SUB["SubtitleRouter + Lifecycle"]
    TR["TranslationDispatcher"]
    OBS["OBS captions (voicesub-obs)"]
  end

  subgraph Browser["Browser Speech"]
    CHR["Chrome/Edge window<br/>/google-asr"]
  end

  subgraph Surfaces["Web surfaces"]
    DASH["Svelte dashboard"]
    OVL["Vanilla overlay"]
    TTS["TTS module /tts"]
  end

  TA <-->|Tauri IPC| RT
  TA --> DASH
  CHR -->|/ws/asr_worker| HTTP
  RT --> CHR
  HTTP -->|/ws/events| DASH
  HTTP -->|/ws/events| OVL
  RT --> SUB --> TR
  SUB --> OBS
  RT --> TTS
```

**Hot path:** `external_asr_update` (WS) → transcript controller → subtitle lifecycle → translation dispatcher → `subtitle_payload_update` / `overlay_update` (WS) → dashboard + overlay.

## 4. Repository Layout

```
F:\AI\VoiceSub\
├── Cargo.toml                  # workspace members, workspace.dependencies
├── Cargo.lock
├── package.json                # Vite/Svelte build scripts
├── vite.config.ts              # → bin/dashboard/
├── vite.worker.config.ts       # → bin/worker/
├── vite.tts.config.ts          # → bin/tts/
├── build-release-msi.bat       # back-compat → build-release.ps1
├── build-release.ps1           # NSIS release pipeline
├── build/release.config.json   # release_root for setup.exe copy
│
├── crates/                     # Rust domain + adapters (see §5)
├── src-tauri/                  # Tauri binary shell (thin)
├── src/                        # Svelte dashboard sources
├── src-worker/                 # Svelte browser worker sources
├── src-tts/                    # Svelte TTS module sources
│
├── bin/                        # Shipped static assets (NSIS resources)
│   ├── dashboard/              # Vite build output
│   ├── worker/                 # Worker bundle
│   ├── tts/                    # TTS UI bundle
│   ├── overlay/                # Vanilla OBS overlay
│   ├── fonts/                  # Project fonts
│   └── modules/                # Sidecar modules (tts)
│
├── tests/
│   ├── golden/                 # Regression fixtures
│   └── integration/
│
├── docs/
├── user-data/                  # runtime (gitignored)
└── logs/                       # runtime (gitignored)
```

### Source vs build artifacts

| Surface | In git | After `npm run build` / installer |
| --- | --- | --- |
| `crates/`, `src/`, `src-worker/`, `src-tts/` | yes | compiled into exe + static |
| `bin/dashboard`, `bin/worker`, `bin/tts` | build output | in NSIS `resources/bin/` |
| `bin/overlay/` | yes | in installer |
| `user-data/`, `logs/` | no | created at runtime |

## 5. Rust Workspace (crates)

Workspace members (`Cargo.toml`): 14 crates + `src-tauri` + `xtask`.

### Dependency graph (simplified)

```
voicesub-types (Layer 0: DTO, WS types, errors)
    ↑
voicesub-config, voicesub-subtitle, voicesub-translation, voicesub-browser,
voicesub-ws, voicesub-logging, voicesub-export, voicesub-obs, voicesub-audio,
voicesub-tts, voicesub-twitch (Layer 1–2)
    ↑
voicesub-runtime (Layer 3: wiring, HTTP router, orchestration)
    ↑
src-tauri (Layer 4: IPC, window, bundle only)
```

### Crate reference

| Crate | Purpose |
| --- | --- |
| `voicesub-types` | `PROJECT_VERSION`, WS envelope types, ASR event DTO |
| `voicesub-config` | TOML store, defaults, legacy JSON import, paths, bind policy |
| `voicesub-subtitle` | `SubtitleLifecycleCore`, `SubtitleRouter`, presentation, overlay contract |
| `voicesub-translation` | `TranslationDispatcher`, `TranslationEngine`, 13 providers |
| `voicesub-browser` | Chrome supervisor, worker launch flags, operational FSM |
| `voicesub-ws` | `/ws/events` hub, `/ws/asr_worker` hub, event sequence |
| `voicesub-http` | Re-export `voicesub-runtime::http` (thin) |
| `voicesub-logging` | `tracing` backbone, rotation, session JSONL, deep trace flags |
| `voicesub-export` | Diagnostics ZIP, config redaction |
| `voicesub-obs` | OBS WebSocket closed captions client |
| `voicesub-audio` | WinAPI audio routing helpers (TTS) |
| `voicesub-tts` | TTS service, queue, Twitch IRC, OAuth bridge |
| `voicesub-twitch` | Twitch IRC (up to 5 channels), emotes, link/symbol filters, Lingua lang detect, `apply_settings` hot-apply |
| `voicesub-runtime` | `RuntimeService`, HTTP router, transcript controller, session wiring |

**Rule:** business logic does not live in `src-tauri/`; Tauri is IPC + lifecycle hooks only.

## 6. RuntimeService: Orchestration and Lifecycle

**File:** `crates/voicesub-runtime/src/service.rs`

`RuntimeService` is the single wiring point:

1. **Start** (`POST /api/runtime/start`):
   - merge optional inline `config_payload`;
   - apply live settings (translation, OBS, subtitle, logging);
   - launch Chrome worker → `{base}/google-asr?autostart=1[&locale=…]`;
   - start translation dispatcher, OBS captions, browser speech ingest;
   - broadcast `preflight_update`, `runtime_update`.

2. **Stop** (`POST /api/runtime/stop`):
   - send `browser_asr_control` stop on `/ws/asr_worker`;
   - kill Chrome process tree (`taskkill /T /F` on Windows);
   - stop translation, OBS; reset subtitle state/metrics.

3. **Tauri shutdown** (`src-tauri/src/lib.rs`):
   - TTS shutdown → `POST /api/runtime/stop` → runtime handle drop.

Embedded HTTP server: dedicated Tokio runtime in Tauri process; bind from `AppConfig` + `VOICESUB_ALLOW_LAN`.

## 7. Configuration and Migrations

### Storage

- **Path:** `{project_root}/user-data/config.toml`
- **Format:** JSON-shaped document serialized as TOML (`voicesub-config::store`)
- **Current version:** `config_version = 8` (`defaults.rs`)

### Top-level keys

| Key | Role |
| --- | --- |
| `config_version` | Schema version (migrate on load) |
| `profile` | Active profile name |
| `ui` | `language`, `layout`, `theme`, `palette`, `show_translation_results` |
| `source_lang` | ASR source (`auto` default) |
| `targets` | Legacy target list (import compatibility) |
| `asr` | `mode` + `browser` tuning |
| `overlay` | `preset`, `compact` |
| `obs_closed_captions` | OBS WebSocket CC settings |
| `translation` | Provider, lines (up to 5), cache, limits, `provider_settings` |
| `subtitle_output` | Source/translation display order |
| `subtitle_lifecycle` | TTL, finalize timing, sync flags |
| `source_text_replacement` | Find/replace pairs for ASR text |
| `logging` | `full_enabled` — master switch for deep diagnostics |

### ASR mode (VoiceSub 0.5.0)

| `asr.mode` | Status |
| --- | --- |
| `browser_google` | **Active default** |
| `browser_google_edge` | Preserved on import; same worker, different page URL |
| `local`, `browser_google_experimental*` (import) | Mapped → `browser_google` + `import_hint` |

### Legacy JSON import (SST Desktop `config.json`)

`ConfigStore::import_sst_json_file` / load with `config_version < 8`:

1. `migrate_sst_payload` — version steps, build `translation.lines` from old `targets`
2. `apply_voicesub_import_rules` — strip removed keys (RNNoise, model fields, …)
3. `repair_legacy_keep_completed_false` + `normalize_config_payload`

Removed providers (e.g. `mymemory`) → fallback `google_translate_v2`.

### Profiles

`user-data/profiles/{name}.toml` — named snapshots via `/api/profiles/*`.

## 8. HTTP API (local)

**Router:** `crates/voicesub-runtime/src/http/router.rs`  
**Default bind:** `127.0.0.1:8765` (`voicesub-config::paths`)  
**LAN:** `VOICESUB_ALLOW_LAN=1` → bind `0.0.0.0`

**LAN security (OWASP ASVS V7):** with `VOICESUB_ALLOW_LAN=1`, HTTP `/api/*` still requires the per-session `x-voicesub-token`, but **WebSocket endpoints remain unauthenticated** — any host on the same network can connect to `/ws/events` (read subtitles/runtime) and `/ws/asr_worker` (send ASR/control). Use LAN bind only on trusted networks; for production streaming prefer default `127.0.0.1` + OBS Browser Source on localhost.

Global middleware: CSP header, `Cache-Control: no-store`.

### Health / Version

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| GET | `/live` | public | Minimal liveness probe (`{"ok":true}`) for OBS overlay |
| GET | `/api/health` | loopback token | Liveness + WS connections + worker connected |
| GET | `/api/version` | loopback token | Product metadata + `sync` (updates config, `update_available`, `latest_known_version`) |

**Loopback API auth:** trusted UI pages (dashboard, worker, TTS) receive a per-session `x-voicesub-token` via HTML injection; Tauri IPC `get_loopback_api_token`. OBS overlay does **not** call protected `/api/*` (only `/live` + WebSocket).

### Devices / OpenAI helpers

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/devices/audio-inputs` | Empty list (browser ASR uses `getUserMedia`) |
| GET | `/api/openai/recommended-models` | Static recommended models |
| POST | `/api/openai/models` | Static list (key not used yet) |
| POST | `/api/openai/usable-models` | Alias |

### Settings / Profiles

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/settings/load` | Config + subtitle presets + font catalog |
| POST | `/api/settings/save` | Merge/save + live apply |
| GET/POST/DELETE | `/api/profiles`, `/api/profiles/{name}` | Profile CRUD |

### Runtime / OBS

| Method | Path | Purpose |
| --- | --- | --- |
| POST | `/api/runtime/start` | Start session (`config_payload?`) |
| POST | `/api/runtime/stop` | Stop session |
| GET | `/api/runtime/status` | Full runtime snapshot |
| GET | `/api/obs/url` | `{ overlay_url }` |

### Logging / Exports

| Method | Path | Purpose |
| --- | --- | --- |
| POST | `/api/logs/client-event` | Client → `session-latest.jsonl` |
| POST | `/api/logs/ui-trace` | UI render trace → `ui-trace.jsonl` |
| GET | `/api/exports` | List export bundles |
| GET | `/api/exports/diagnostics` | Diagnostics ZIP |

### TTS / Twitch OAuth

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/tts/google` | Google Translate TTS proxy |
| GET | `/api/tts/python` | TTS via embedded Python module |
| GET | `/api/tts/python/status` | Python runtime probe |
| POST | `/api/tts/twitch/oauth-open` | Open Twitch OAuth in system browser |
| GET | `/api/tts/twitch/oauth-pending` | Poll pending token |
| POST | `/api/tts/twitch/oauth-complete` | Store OAuth token |

### Updates

| Method | Path | Purpose |
| --- | --- | --- |
| POST | `/api/updates/check` | Poll GitHub Releases (forced on dashboard bootstrap); persists `updates.latest_known_version`, `last_checked_utc` |

### HTML pages

| Method | Path | Handler |
| --- | --- | --- |
| GET | `/` | `bin/dashboard/index.html` |
| GET | `/overlay` | `bin/overlay/overlay.html` |
| GET | `/google-asr` | `bin/worker/index.html` |
| GET | `/google-asr-edge` | Same worker bundle |
| GET | `/tts` | `bin/tts/index.html` |
| GET | `/project-fonts.css` | Generated `@font-face` from `bin/fonts/` |

### Static mounts

| URL prefix | Disk path |
| --- | --- |
| `/overlay-assets` | `bin/overlay/` |
| `/static` | `bin/overlay/shared/` (legacy shared assets) |
| `/worker-assets` | `bin/worker/` |
| `/assets` | `bin/dashboard/assets/` |
| `/tts-assets` | `bin/tts/` |
| `/project-fonts` | `bin/fonts/` |

`bin/` resolved via `ProjectPaths::locate_bin_dir()` — workspace `bin/` or Tauri NSIS `resources/bin/`.

## 9. WebSocket Surface

**Authentication:** WS endpoints do **not** use `x-voicesub-token` (by design — OBS overlay and browser worker). With default bind `127.0.0.1` risk is limited to the local machine. With `VOICESUB_ALLOW_LAN=1` see the warning in §8.

### `/ws/events` — OBS overlay (+ optional external clients)

**Implementation:** `crates/voicesub-ws/src/events.rs`

- Client receive-only (inbound text ignored)
- On connect: `hello` (`type: "hello"`, `message: "connected"`)
- Replay last: `runtime_update`, `subtitle_payload_update`, `overlay_update`
- Bounded per-socket queue (default 128), dedupe by `type`

**Envelope:** `{ "type": "<channel>", "payload": {…} }`  
Payload enrichment: `event_sequence`, `created_at_ms`, `event_type` (`WsEventPublisher`).

| `type` | Purpose |
| --- | --- |
| `hello` | Handshake |
| `runtime_update` | Phase, ASR/worker state, metrics |
| `preflight_update` | `{ running: bool }` during start/stop |
| `diagnostics_update` | ASR diagnostics snapshot |
| `model_status_update` | Model/ASR readiness |
| `transcript_update` | ASR partial/final events |
| `transcript_segment_event` | Segment-level duplicate channel |
| `subtitle_payload_update` | Subtitle presentation (replay on connect) |
| `overlay_update` | Overlay render body (replay on connect) |
| `translation_update` | Per-sequence translation results |
| `twitch_chat_message` | Twitch chat for TTS |
| `twitch_connection_update` | Twitch connection state |

**Stale guard:** overlay (`overlay.js` + `ws-stale-guard-logic.js`) drops stale events after stop/start (timestamp-first on sequence reset).

### In-process runtime events — Tauri dashboard + TTS (0.5.2+)

**Implementation:** `RuntimeEventBus` (`crates/voicesub-ws/src/event_bus.rs`) + Tauri emit `runtime-event` (`src-tauri/src/lib.rs`).

- Main dashboard (`src/lib/runtime-events.ts`) and TTS module (`src-tts/App.svelte`) **do not** open `ws://127.0.0.1:8765/ws/events`; they receive the same `{ type, payload }` envelopes via Tauri events.
- On subscribe: IPC `get_runtime_state_snapshot` replays runtime/subtitle/overlay/translation/diagnostics without WS handshake race.
- `WsEventPublisher` mirrors broadcasts into the EventBus for shell clients; OBS overlay remains WS-only.

**Legacy:** `src/lib/ws.ts` (`EventsSocket`) for dev/external browser clients; production Tauri shell uses `runtime-events.ts`.

### `/ws/asr_worker` — browser worker

**Implementation:** `crates/voicesub-ws/src/asr_worker.rs`

**Server → worker:**

| `type` | Fields | Purpose |
| --- | --- | --- |
| `hello` | `message: "browser_asr_worker_connected"`, `transport_id` | Handshake |
| `browser_asr_control` | `action`, `reason?`, `issued_at_ms`, `transport_id` | Control (e.g. `stop`) |

**Worker → server:**

| `type` | Handler |
| --- | --- |
| `external_asr_update` | ASR text ingest (partial/final, generation guards) |
| `browser_asr_status` | Worker state snapshot |
| `browser_asr_heartbeat` | Same as status |
| `hello` | Recognized, no special handling |

## 10. Tauri IPC

**Registration:** `src-tauri/src/lib.rs` → `tauri::generate_handler!`

### Shell commands

| Command | Purpose |
| --- | --- |
| `voicesub_version` | Returns `PROJECT_VERSION` |
| `set_dashboard_layout` | Compact (390×844) vs standard (1280×900) window |
| `launch_browser_worker` | Launch Chrome to worker URL without full runtime start |

### TTS commands (`src-tauri/src/tts.rs`)

| Command | Purpose |
| --- | --- |
| `tts_get_config` | Load TTS config |
| `tts_set_provider` / `tts_set_enabled` | Provider toggle |
| `tts_set_audio_device` / `tts_set_channel_audio_device` | Speech / Twitch audio output |
| `tts_set_playback_mode` | `native` (cpal @ 1.0×) or `sonic` (libsonic); legacy `browser` → `sonic` on load |
| `tts_play_audio` / `tts_stop_channel` | Native MP3 playback per channel |
| `tts_list_output_devices` | WASAPI enumeration (label-first for native) |
| `tts_get_audio_routing` / `tts_bind_window_audio` | Legacy WinAPI per-process routing (single device) |
| `tts_update_speech_settings` / `tts_update_voice_settings` | Speech params |
| `tts_plan_subtitle_speech` / `tts_reset_subtitle_planner` | Subtitle-driven queue |
| `tts_channel_enqueue` | Enqueue text on `speech` or `twitch` channel (preferred) |
| `tts_enqueue` | **Deprecated** — backward-compat speech enqueue; prefer `tts_channel_enqueue` |
| `tts_twitch_*` | Twitch connect/disconnect/status/settings |
| `tts_sync_source_text_replacement` | Sync replacement rules |
| `tts_open_window` | Open/focus `/tts` webview |
| `tts_open_system_url` | Open validated Twitch OAuth URL externally |
| `open_external_https_url` | Open GitHub release page (update banner **Download**) in system browser |
| `get_runtime_state_snapshot` | Replay runtime/subtitle/overlay/translation/diagnostics for Tauri shell on connect |

**Tauri events (shell clients):** `runtime-event` (WS-shaped envelopes), `tts-speech-activity` (planned speech queue items for TTS UI log).

**Lifecycle:** main webview → `http://{bind_addr}/` on setup; on close → TTS shutdown → runtime stop.

## 11. Logs, Diagnostics, Export

**Directory:** `{project_root}/logs/`

### Backbone (always)

| File | Purpose |
| --- | --- |
| `core.log` | `tracing` backbone (+ stderr); rotate → `core.old.log` on startup |
| `runtime-events.log` | Compact structured events (5 MB rotation) |
| `session-latest.jsonl` | Client events from `/api/logs/client-event` (max 5000 lines) |

### Opt-in JSONL traces

Master switch: `logging.full_enabled` in config **or** `VOICESUB_DEEP_DIAGNOSTICS`.

| File | Enable env |
| --- | --- |
| `subtitle-trace.jsonl` | `VOICESUB_TRACE_SUBTITLE` |
| `tts-trace.jsonl` | `VOICESUB_TRACE_TTS` |
| `browser-trace.jsonl` | `VOICESUB_TRACE_BROWSER` |
| `obs-trace.jsonl` | `VOICESUB_TRACE_OBS` |
| `ui-trace.jsonl` | `VOICESUB_TRACE_UI` |
| `ws-trace.jsonl` | `VOICESUB_TRACE_WS` |
| `pipeline-trace.jsonl` | `VOICESUB_TRACE_PIPELINE` |
| `session-lifecycle.json` | always (session marker); shutdown/panic steps also in `pipeline-trace.jsonl` when deep diagnostics on |

Disable: same vars `=0` / `false`.  
Verbose runtime-events: `VOICESUB_TRACE_RUNTIME_EVENTS_VERBOSE`.

With `logging.full_enabled`, close steps (`shutdown_begin`, `shutdown_step`, `shutdown_complete`) go to `core.log` (`voicesub.lifecycle`) and `pipeline-trace.jsonl`. `session-lifecycle.json` is always updated: `running` → `graceful` or `panic`. If the next start still finds `running`, `core.log` gets `previous session exited without graceful shutdown` (even in compact mode).

### Other env vars

| Variable | Purpose |
| --- | --- |
| `VOICESUB_ALLOW_LAN` | Bind `0.0.0.0` |
| `RUST_LOG` | `tracing` filter override |
| `VOICESUB_TTS_PER_PROCESS_ROUTING` | WinAPI TTS audio routing |
| `VOICESUB_TTS_ALLOW_SYSTEM_PYTHON` | Allow system Python for TTS fetcher |

### Diagnostics ZIP

`GET /api/exports/diagnostics` bundles: `runtime_status.json`, `config_redacted.json`, `environment.txt`, `latest_session.jsonl`, `core.log`, `runtime-events.log`.

## 12. Browser Speech Worker

### URL and launch

| Constant | Value |
| --- | --- |
| `WORKER_PATH` | `/google-asr` |
| Edge alias | `/google-asr-edge` |
| Launch URL | `{base}/google-asr?autostart=1[&locale={ui.language}]` |

`worker_launch_browser`: `auto` | `google_chrome` (unknown → `auto`).

### Chrome launch invariants

- **Separate** Chrome/Edge window with **visible address bar**
- Isolated `--user-data-dir`: `{user-data}/browser-worker-profile-classic-{engine}/`
- Edge: `--disable-sync`, `--allow-browser-signin=false`; **never** `--disable-extensions` / `--bwsi`
- **No** `--app`, hidden windows, in-tab worker
- Anti-throttling Chrome flags + Windows EcoQoS opt-out (`launch_config.rs`, `ecoqos.rs`)
- Detached high-priority process; stop via `taskkill /T /F` (only when real `pid > 0`)
- **Launch stability (0.5.2+):** `launch_stability.rs` (flag profile), `profile_bloat_guard.rs` (profile dir hygiene), `process_affinity.rs` (Windows CPU affinity); contract tests in `crates/voicesub-browser/tests/chrome_launch_contract.rs`

### Test harness (no Chrome spawn)

- `voicesub-browser::browser_worker_launch_skipped()` — `cfg(test)` in crate unit tests + env `VOICESUB_SKIP_BROWSER_WORKER=1`
- Integration tests (`voicesub-http/tests/`, `voicesub-runtime/tests/`) set skip in `integration_lock()` — dependencies build **without** `cfg(test)`
- Stub launch: `pid: 0`, `worker_pid = None`; optional `VOICESUB_FORCE_BROWSER_WORKER=1` for manual verification

### Worker frontend (`src-worker/`)

| Module | Role |
| --- | --- |
| `worker-controller.ts` | Autostart, recognition lifecycle |
| `socket-bridge.ts` | `/ws/asr_worker` connect, `browser_asr_control` |
| `session-manager.ts` | Session age, reconnect, watchdog |
| `web-speech-policy.ts` | Strip on-device hints, overlap policy |

**Defaults:** lang `ru-RU`, interim/continuous on, force-finalization 1600ms, max session age 180s.

## 13. Translation: Lifecycle and Invariants

**Crate:** `voicesub-translation`  
**Entry:** `TranslationDispatcher` (`dispatcher.rs`)

### Providers (13)

`SUPPORTED_PROVIDERS` in `providers/mod.rs`:

| ID | Group |
| --- | --- |
| `google_translate_v2` | API (default) |
| `google_cloud_translation_v3` | API |
| `google_gas_url` | API |
| `google_web` | experimental |
| `azure_translator` | API |
| `deepl` | API |
| `libretranslate` | API/self-hosted |
| `openai` | llm |
| `openrouter` | llm |
| `lm_studio` | local_llm |
| `ollama` | local_llm |
| `public_libretranslate_mirror` | API |
| `free_web_translate` | experimental |

Up to **5 translation lines** (`translation_1`…`translation_5`). Test stub `stub` — not in production registry.

### Critical lifecycle invariant (non-negotiable)

- Completed subtitle block **stays on screen** until a **new** phrase is finalized
- Late translations **allowed** (no wall-clock stale drop on browser path)
- Preview supersession by `(segment_id, revision)`
- Stale drop for superseded **in-flight** jobs on new segment/revision

## 14. Subtitle Lifecycle and Presentation

**Crate:** `voicesub-subtitle`

| Component | File | Role |
| --- | --- | --- |
| `SubtitleLifecycleCore` | `lifecycle.rs` | FSM, TTL, relevance, expiry scheduling |
| `SubtitleRouter` | `router.rs` | Transcript + translation → presentation events |
| `SubtitlePresentation` | `presentation.rs` | Payload assembly |
| Overlay contract | `tests/overlay_contract.rs` | Golden regression |

**Config keys (`subtitle_lifecycle`):**

- `completed_block_ttl_ms` (default 4500, min 500)
- `completed_source_ttl_ms`, `completed_translation_ttl_ms`
- `pause_to_finalize_ms`, sync flags

**Router actor** (`router_actor.rs`) — async publish path with `subtitle_payload_update` + `overlay_update` fanout.

## 15. Subtitle Styles and Overlay

### Backend config

Subtitle style presets loaded via `/api/settings/load` together with config. Font catalog from `bin/fonts/` + `project-fonts.css`.

### Overlay presets

`overlay.preset`: `single` | `dual-line` | `stacked` | `compact`  
Query param override: `?preset=…&compact=1&profile=…&debug=…`

### Shared renderer

`bin/overlay/shared/js/subtitle-style.js` — fast/slow path invariants. Dashboard preview uses the same payload shape via WS (not necessarily the same JS file).

### OBS overlay URL (VoiceSub 0.5.0)

```
http://127.0.0.1:8765/overlay
```

**Query-param backward compatibility with older products is not guaranteed.** Users update OBS Browser Source manually when overlay URL or params change.

### Empty-state cleanup (caller responsibility)

After fast-path optimizations the renderer keeps DOM/state across frames. On an empty payload (TTL expiry, Stop, `lifecycle_state: idle`) the caller **must** call `disposeRenderContainer`:

| Surface | Caller |
| --- | --- |
| Dashboard preview | `src/lib/components/SubtitleOutputPreview.svelte` |
| OBS overlay | `bin/overlay/overlay.js` — after `render()`, when `result?.empty` |

Without cleanup the last subtitle frame can stick in OBS. Contract: `crates/voicesub-subtitle/tests/overlay_contract.rs` → `overlay_disposes_renderer_when_payload_is_empty`.

## 16. OBS Closed Captions

**Crate:** `voicesub-obs`  
**Config:** `obs_closed_captions` in config

- OBS WebSocket v5 client (`host`, `port`, `password`)
- `output_mode`: `disabled` | `source_live` | `source_final_only` | `translation_1..5` | `first_visible_line`
- `debug_mirror` — optional OBS Text Source mirror (`SetInputSettings`)
- `timing` — partial throttle, final replace delay, clear after ms, dedup
- Two inputs: ASR **source events** (`source_live` / `source_final_only`) and **subtitle payload** (`translation_*`, `first_visible_line`, debug mirror)
- Send/clear/dedup algorithm with 0.5.2 fixes (501 debug clear, supersede generation, partial native stop after 501)

Enabled when `obs_closed_captions.enabled = true` and connection succeeds. Native `SendStreamCaption` only during an active stream.

## 17. TTS Module

Shipped as **module** under `bin/modules/tts/` + Svelte UI at `/tts`.

### Manifest

`bin/modules/tts/module.toml` — `entry_url_path = "/tts"`, requires core `>=0.5.0`.

### Components

| Layer | Path |
| --- | --- |
| UI | `src-tts/` → `bin/tts/` |
| Rust service | `crates/voicesub-tts/` |
| Native playback | `crates/voicesub-audio/src/playback.rs` (`PlaybackHub`) |
| Twitch | `crates/voicesub-twitch/` |
| Python sidecar | `bin/modules/tts/runtime/win-x64/google_tts_fetch.exe` |

### UI tabs

`speech` | `twitch` (`src-tts/lib/types.ts`)

### Dual sink (speech + twitch) — Rust hot path (0.5.2+)

Two independent playback channels with separate Rust queues and WASAPI devices:

| Channel | Source | Orchestrator | Config device fields |
| --- | --- | --- | --- |
| `speech` | `subtitle_payload` → `TtsSpeechPipeline` | `ChannelOrchestrator` (speech) | root `audio_output_device_*` |
| `twitch` | IRC → `TwitchChatService` | `ChannelOrchestrator` (twitch) | `[twitch].audio_output_device_*` |

Live path: plan → **`google_fetch.rs`** (HTTP) → enqueue → prefetch → `PlaybackHub` (`tts_play_audio`). TTS WebView — settings UI + manual sample test only (`SpeechEngine` preview).

**0.5.1 (legacy note):** queue/prefetch lived in `src-tts/lib/speech-engine.ts` + `google-tts.ts`; **0.5.2** moved the hot path to Rust (`speech_pipeline.rs`, `channel_orchestrator.rs`).

### Playback modes (`playback_mode` in `user-data/modules/tts/config.toml`)

| Mode | Mechanism | When |
| --- | --- | --- |
| `native` (default) | `PlaybackHub` (cpal) @ 1.0×, IPC `tts_play_audio` | Lowest latency |
| `sonic` | libsonic tempo stretch, pitch-preserving rate | Queue / rate boost |
| `browser` (legacy) | — | **Migrates to `sonic`** on config load |

Tauri event: `playback-finished` `{ channel, item_id, ok, error? }`.

Devices: **label-first** (WASAPI friendly name → `cpal::Device`). List via `tts_list_output_devices`.

### Twitch IRC and filters (`voicesub-twitch`)

| Aspect | Behavior |
| --- | --- |
| Channels | Up to **5** logins in `TwitchTtsSettings.channels`; IRC `JOIN #a,#b,…`; legacy `channel` → `channels[0]` |
| Hot-apply | `TwitchChatService.apply_settings()` on `tts_update_twitch_settings` — no reconnect for filter changes |
| Reconnect | `run_session_with_reconnect()` — auto-retry on stream/TCP/TLS loss; backoff 1→30 s; auth/settings errors stop the loop |
| Emotes | Twitch IRC tag + BTTV/7TV/Twitch lexical; **pure numeric tokens** are not matched as emote codes |
| Emoji strip | `strip_unicode_emoji` preserves decimal digits (ASCII / Arabic-Indic / Fullwidth); `\p{Emoji}` does not eat `0–9` in chat text |
| Links | `links.rs` + double `strip_links` in pipeline; link-only → `speakable: false` |
| Symbols | `strip_symbols` — comma-separated tokens (default `@, &, $, _`); optional `replace_underscore_with_space` for nick/text |
| Lang | Lingua 1.8 subset + Unicode heuristics + whatlang; `strip_leading_speaker_label` |
| UI | `TwitchPanel.svelte`: connection card, save queue (`saveNow` / debounce), “Settings applied” badge, `?` nick help (`popover-position.ts`) |

Config: `user-data/modules/tts/config.toml` → `[twitch]` section.

### Legacy audio routing

- WinAPI per-process routing: `VOICESUB_TTS_PER_PROCESS_ROUTING` + `tts_bind_window_audio` — one device per WebView process; **do not use** for dual sink (use native/Sonic `PlaybackHub` instead).

## 18. Desktop Runtime and NSIS Release

### Tauri config

`src-tauri/tauri.conf.json`:

- `productName`: VoiceSub
- `identifier`: `com.voicesub.app`
- `frontendDist`: `../bin/dashboard`
- `beforeBuildCommand`: `npm run build`
- Bundle: **NSIS** (`targets: ["nsis"]`, `installMode: currentUser`, languages en/ru/ja/ko/zh)
- NSIS template: `src-tauri/windows/installer.nsi`, hooks: `src-tauri/windows/hooks.nsh`
- WebView2: `downloadBootstrapper` (silent=false)
- Resources: `bin/dashboard`, `overlay`, `worker`, `tts`, `fonts`, `modules`

Legacy WiX `src-tauri/wix/main.wxs` — **not used** (reference only).

### Release pipeline

```
build-release-msi.bat          # back-compat entry
  → build-release-msi.ps1
  → build-release.ps1
    1. npm run build (+ build:tts)
    2. bin\modules\tts\build_runtime.bat (if google_tts_fetch.exe missing)
    3. node scripts/validate-nsis-i18n.mjs
    4. cargo tauri build (NSIS)
    5. Copy VoiceSub_{version}_x64-setup.exe → release_root/v{version}/
```

Default `release_root`: `F:\AI\VoiceSub - release\v{version}\`

### Install layout

- Per-user install (`currentUser`) — typically `%LOCALAPPDATA%\Programs\VoiceSub\`
- `user-data/` and `logs/` — next to install dir / project root (`ProjectPaths`)

### Dev workflow

- `npm run dev` — Vite dashboard on port 5173 (optional; production path uses embedded server)
- Tauri loads `http://127.0.0.1:8765` (Axum serves built dashboard)

**End user install:** NSIS `setup.exe` only. No Python/Node/torch in core installer. Chrome is a system dependency for Web Speech.

## 19. Storage and Paths

| Path | Purpose |
| --- | --- |
| `user-data/config.toml` | Main config |
| `user-data/profiles/` | Named profiles |
| `user-data/browser-worker-profile-classic-*/` | Chrome isolated profiles |
| `user-data/translation-cache/` | Persistent translation cache |
| `logs/` | Runtime logs |
| `bin/` | Shipped static (workspace or NSIS resources) |

`ProjectPaths::discover(project_root)` resolves all paths relative to project root or Tauri resource dir.

## 20. Frontend: Dashboard (Svelte)

**Sources:** `src/`  
**Build:** `vite.config.ts` → `bin/dashboard/`

### Navigation

Single-page **tab switch** (no SvelteKit router):

| Tab ID | Panel |
| --- | --- |
| `translation` | `TranslationPanel.svelte` |
| `subtitles` | `SubtitlesPanel.svelte` |
| `style` | `StylePanel.svelte` |
| `theme` | `ThemePanel.svelte` |
| `obs` | `ObsPanel.svelte` |
| `replacement` | `ReplacementPanel.svelte` |
| `tools` | `ToolsPanel.svelte` |
| `settings` | `SettingsPanel.svelte` |
| `help` | `HelpPanel.svelte` |

Compact layout adds pane `"live"` (overview).

### Key libs

| File | Role |
| --- | --- |
| `src/lib/api.ts` | REST client |
| `src/lib/ws.ts` | `/ws/events` + stale guard |
| `src/lib/stores/app.ts` | App state |
| `src/lib/config-*.ts` | Config normalize/save |

### Layout IPC

`set_dashboard_layout` Tauri command — compact vs standard window sizes.

### Idle subtitle preview (before Start)

**Files:** `src/lib/preview-payload.ts`, `src/lib/components/SubtitleOutputPreview.svelte` (embedded from `OverviewSection.svelte`)

While runtime is in `idle` phase, the dashboard shows **placeholder preview** (`preview.source_line`, translation labels) instead of live `overlay_update` from WS. An empty `overlay_update` after Save **does not clear** the preview. When `running=true`, preview switches to live payload (`subtitle_payload_update` / `overlay_update`). Test: `src/lib/preview-payload.test.ts`.

## 21. Frontend: Overlay (vanilla)

**Path:** `bin/overlay/`

| File | Role |
| --- | --- |
| `overlay.html` | Shell |
| `overlay.js` | WS consumer, render loop; `disposeRenderContainer` on empty |
| `overlay.css` | Styles |
| `shared/js/subtitle-style.js` | Renderer |
| `shared/js/core/ws-stale-guard-logic.js` | Stale filter |
| `shared/js/i18n/` | Overlay i18n bundle |

**WS:** `ws(s)://{host}/ws/events` — handles `transcript_update`, `overlay_update`.  
**Reconnect:** exponential backoff 1s → 10s max; last frame preserved on disconnect (OBS UX).  
**Debug:** `?debug=1` gates `writeDebug` buffer + `console.debug`; `?debug-subtitles=1` enables subtitle-effect trace ring (`window.__sstOverlaySubtitleTrace`). No production `console.log` on hot path.  
**Empty payload:** `disposeRenderContainer(linesContainer)` when render returns `empty: true` (TTL / Stop / idle). Idle TTL also requires `hasVisibleRenderedFrame()` so state-only clear does not skip DOM teardown. Cache-bust: `overlay.html` → `overlay.js?v=20260615a`.

## 22. Frontend: Browser Worker (Svelte)

**Sources:** `src-worker/`  
**Build:** `vite.worker.config.ts` → `bin/worker/` (`base: "/worker-assets/"`)

Entry: `main.ts` → `WorkerApp.svelte`  
Autostart: `?autostart=1` query param.

## 23. UI Localization (i18n)

**Locales:** `en`, `ru`, `ja`, `ko`, `zh`

| Surface | Catalog location |
| --- | --- |
| Dashboard | `src/lib/i18n/locales/{locale}.json` + `tts-{locale}.json` |
| Overlay | `bin/overlay/shared/js/i18n/` |
| Worker | via `locale` query param + worker i18n |

Merge: `src/lib/i18n/index.ts` — main + TTS catalogs per locale.  
**Source of truth:** `scripts/i18n-source/locales/*.js` + `dynamic-locales.js`.  
Export pipeline: `npm run i18n:export` → `scripts/export-i18n.mjs` → `src/lib/i18n/locales/*.json`.  
Overlay bundle: `npm run i18n:bundle` → `scripts/build-locale-bundle.mjs` → `scripts/i18n-source/locales-bundle.js` + `bin/overlay/shared/js/i18n/`.  
Config key: `ui.language` (empty = browser default).

## 24. Versioning and Update Checks

- **Single source (interim):** `voicesub-types::PROJECT_VERSION` = `"0.5.3"`
- Workspace `Cargo.toml` `[workspace.package].version` = `0.5.3`
- `package.json`, `tauri.conf.json` — aligned `0.5.3`
- `GET /api/version`, `POST /api/updates/check` — GitHub Releases poll (`update_service.rs`, `voicesub-types::version`)
- Config `updates.*` — defaults + `normalize_updates_config` for legacy `config.toml`
- Dashboard `UpdateBanner.svelte`; **Download** → Tauri `open_external_https_url` (`shell.rs`)

## 25. Testing

### Policy

- **No new Rust module without tests** in the same task
- Golden fixtures in `tests/golden/` — update when behavioral contracts change
- `cargo test --workspace` required before done

### Levels

| Level | Where | What |
| --- | --- | --- |
| Unit | `crates/*/src/**` | FSM, stale drop, normalization |
| Golden | `tests/golden/` + crate `tests/golden_*.rs` | Payload parity |
| Integration | `tests/integration/`, `voicesub-http/tests/` | HTTP/WS smoke |
| Frontend | `npm run test:frontend` (Vitest) | i18n, normalizers, worker, `popover-position`, `twitch-channels` |

### Key test files

- `voicesub-subtitle/tests/golden_subtitle.rs`, `golden_ttl_lifecycle.rs`
- `voicesub-translation/tests/golden_translation.rs`, `golden_stale_translation.rs`
- `voicesub-http/tests/http_ws_smoke.rs` — runtime start **without** Chrome (`VOICESUB_SKIP_BROWSER_WORKER`)
- `voicesub-twitch` — pipeline/links/lang/emoji digits/emotes/`apply_settings` (105+ unit tests)
- `voicesub-browser/tests/worker_svelte_contract.rs`, `launcher.rs` launch skip
- `voicesub-subtitle/tests/overlay_contract.rs` — overlay lifecycle + empty cleanup
- `src/lib/preview-payload.test.ts`, `tests/renderer/dashboard-panel.contract.test.ts` — idle/live preview + `SubtitleOutputPreview` renderer contract

## 26. Product Invariants

1. **Local-first:** default localhost bind; no cloud assumptions.
2. **Browser worker visibility:** separate window, visible URL bar, no hidden/throttled-to-death modes.
3. **Subtitle lifecycle:** completed block persists until new phrase finalized; late translations allowed on browser path.
4. **Translation:** 13 providers, full dispatcher semantics (queue, stale drop, supersession).
5. **Overlay separation:** vanilla HTML for OBS; not bundled in dashboard Vite chunk.
6. **No Node in runtime:** only compile-time frontend toolchain.
7. **Config import:** legacy SST `config.json` import preserves user intent except explicitly removed modes.

## 27. Known Limitations & Technical Debt

### 27.1 Current limitations

- GitHub update **check + dashboard banner** implemented; installer auto-download not implemented (opens release page in system browser)
- `POST /api/openai/models` — static list; live OpenAI model fetch deferred
- Audio input enumeration empty (by design for browser ASR)

### 27.2 Technical debt

- `PROJECT_VERSION` scattered across Cargo/package/tauri — migrate to crate-only source of truth

## 28. Security & Privacy Model

- **Bind policy:** localhost default; LAN only via explicit `VOICESUB_ALLOW_LAN=1`
- **CSP** on all HTTP responses (restrictive `default-src 'self'`)
- **Diagnostics export:** config redaction before ZIP
- **No telemetry** to vendor servers by default
- Translation provider API keys stored locally in `config.toml` / `provider_settings`
- Twitch OAuth tokens stored locally in TTS bridge
- Browser worker uses isolated Chrome profile (no sync)

## 29. Extension Points

### Safe extension

| Extension | How |
| --- | --- |
| New translation provider | Add to `voicesub-translation/src/providers/`, register in `mod.rs`, golden tests |
| New WS event type | Add to `voicesub-ws`, document in §9, update dashboard/overlay consumers |
| New config key | `voicesub-config` defaults + migrate + normalize + TECH_ARCH §7 |
| New module | `bin/modules/{name}/module.toml` + sidecar |
| Dashboard panel | New `src/lib/panels/*.svelte` + register in `navigation.ts` (`NavRail` / `BottomNav`); optional `PanelListDetailLayout` for long panels |

### Unsafe (forbidden without contract update)

- Changing subtitle lifecycle semantics
- Adding Node.js to runtime
- Reintroducing experimental routes in core HTTP server
- Business logic in `src-tauri/`

## 30. Glossary

| Term | Meaning |
| --- | --- |
| **ASR** | Automatic Speech Recognition |
| **Browser worker** | Chrome/Edge window running Web Speech at `/google-asr` |
| **Completed block** | Finalized subtitle segment shown until next phrase finalizes |
| **Golden test** | Fixture-based regression test |
| **Overlay** | Vanilla OBS Browser Source page at `/overlay` |
| **Segment / revision** | Translation supersession identity `(segment_id, revision)` |
| **Sidecar module** | Optional feature (TTS) under `bin/modules/` |
| **Stale drop** | Discarding in-flight translation superseded by newer segment |
| **VoiceSub** | Product name for the 0.5.x line |
