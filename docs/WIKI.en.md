# VoiceSub — WIKI

Operational guide for the VoiceSub `0.6.0` UI. Each element is described as: **what it is**, **why it exists**, **how it works**, **what it affects**, and **common mistakes**.

Technical architecture: `docs/TECHNICAL_ARCHITECTURE.en.md`. SST `0.4.4` is a frozen predecessor; core behavior in this document is VoiceSub-specific.

---

## 0. About the product

### Element: VoiceSub vs SST
- **VoiceSub** — active `0.6.0` line (Rust + Tauri + Svelte); first release baseline is `0.5.0`.
- **SST** `0.4.4` — frozen reference; settings import works, but legacy local ASR and experimental browser modes are not started in core.
- **New overlay URL:** `http://127.0.0.1:8765/overlay` — update OBS Browser Source manually.

### Element: system requirements
- **Windows 10/11 x64**
- **WebView2 Runtime** — required for `VoiceSub.exe` (Tauri dashboard shell, `/tts`, `/local-asr`). The app will not start without it. Usually present on Windows 11; on Windows 10 the installer may offer the bootstrapper.
- **Google Chrome** — separate dependency for the Web Speech worker window (`/google-asr`); not the same as WebView2. Not required when using Local ASR only.
- Microphone in the Chrome worker **or** via Local ASR native capture; internet for external translation providers (optional) and first-time Local ASR downloads.

### Element: install and update (NSIS)
- **What it does:** `VoiceSub_0.6.0_x64-setup.exe` installs `VoiceSub.exe` and bundled static assets (dashboard, overlay, worker, tts, local-asr).
- **Why:** single installer without Python/Node in runtime; downloads WebView2 via bootstrapper when missing (`downloadBootstrapper` in Tauri).
- **Update:** close app → run new `setup.exe` over existing → `user-data/` and `logs/` persist next to install/project root.
- **Developers:** `build-release-msi.bat` → `build-release.ps1` → `F:\AI\VoiceSub - release\v{version}\`.
- **Update check:** on dashboard bootstrap the app polls GitHub Releases (`POST /api/updates/check`). When a newer tag exists, a top banner appears (“VoiceSub X is available — you are on Y”). **Download** opens the release page in the system browser. Config: `user-data/config.toml` → `[updates]` (`enabled`, `github_repo`, `check_interval_hours`). The section is merged automatically when upgrading from SST.

### Element: local URLs
| URL | Purpose |
| --- | --- |
| `/` | Svelte dashboard |
| `/overlay` | OBS Browser Source |
| `/google-asr` | Browser Speech worker |
| `/tts` | TTS module UI |
| `/local-asr` | Local ASR module UI |

---

## 1. Quick start

### Element: first run
1. Launch **VoiceSub.exe**.
2. Dashboard opens in the Tauri main window (`http://127.0.0.1:8765/`).
3. Add OBS Browser Source: `http://127.0.0.1:8765/overlay`.
4. Configure UI language (Settings) and translation (Translation) if needed.
5. Click **Start** — Chrome opens `/google-asr?autostart=1` (Web Speech) **or** Local ASR starts in-process when `local_parakeet` is selected and ready.
6. Grant microphone permission in Chrome (Web Speech) **or** pick a mic in the Local ASR module, then speak.

### Element: runtime bar (Start / Stop)
- **Start:** `POST /api/runtime/start` — worker, translation, OBS CC, ASR ingest.
- **Stop:** stops worker (kills Chrome tree), resets subtitle state.
- **Note:** Start sends the current config snapshot, including unsaved edits since last Save.

### Element: subtitle preview (overview)
- **What it does:** top **Subtitle Output Preview** shows placeholders before Start and live payload after.
- **Why:** style calibration without running ASR; empty post-save `overlay_update` does not clear preview.
- **Details:** `TECHNICAL_ARCHITECTURE.en.md` §21 (Idle subtitle preview).

### Element: compact layout
- **What it does:** switches Tauri window (~390×844) and navigation with **Live** pane + settings tabs.
- **Where:** layout button in chrome or command palette.
- **IPC:** Tauri `set_dashboard_layout`.

---

## 2. Troubleshooting

### Scenario: no text at all
- Is runtime **Start**ed?
- **Web Speech:** is Chrome `/google-asr` window open and **visible**? Microphone allowed in **Chrome**?
- **Local ASR:** is mode `local_parakeet` selected and module `ready`? Mic selected in `/local-asr`?
- **Tools & Data** → runtime diagnostics: `browser_worker_connected` (Web Speech) or Local ASR status?

### Scenario: source text but no translation
- **Translation** tab → translation enabled.
- At least one `translation_N` line with `enabled`.
- Check translation results / diagnostics for provider errors.

### Scenario: OBS shows nothing
- Browser Source URL is `/overlay` (not dashboard `/`).
- **Subtitles** → source/translation visibility enabled.
- TTL not too aggressive (text may flash and vanish).
- On WS disconnect overlay keeps last frame (stale-guard + 1–10 s backoff) — expected.
- Overlay dev console noise — use `?debug=1` only when debugging; production hot path does not log every WS frame.
- Text **stuck after TTL/Stop** — update the app and reload Browser Source (`disposeRenderContainer` + `hasVisibleRenderedFrame`, `overlay.js?v=20260615a`).

### Scenario: worker keeps dying
- Check network (Web Speech uses Google endpoints).
- `logs/browser-trace.jsonl` with `VOICESUB_TRACE_BROWSER=1`.
- Recovery: **Stop** → **Start** or relaunch worker from Tools.

---

## 3. Dashboard tabs

| Tab | Purpose |
| --- | --- |
| **Translation** | Providers, translation lines, cache, dispatcher limits |
| **Subtitles** | Overlay preset, visibility, order, TTL lifecycle |
| **Style** | Fonts, colors, effects, slot styles, custom presets |
| **UI Theme** | Dark/light mode, accent palette |
| **OBS** | Overlay URL, Closed Captions, debug mirror |
| **Word Replace** | Text replacement before translation |
| **Tools & Data** | Profiles, diagnostics, ZIP export |
| **Settings** | UI language, layout, SST config import, Web Speech advanced |
| **Help** | Built-in help topics |

**Command palette** (header search / `Ctrl+K`): quick navigation, Start/Stop, Save, export diagnostics.

---

## 4. Recognition (Browser Speech)

### Element: sole production mode in core 0.5.0
- **`browser_google`** — Web Speech in a separate Chrome window.
- Microphone is selected **in Chrome** (`getUserMedia`), not in the dashboard.
- `/api/devices/audio-inputs` returns empty — by design.

### Element: browser worker window
- **Separate window** with **visible address bar** (no app mode, no hidden tab).
- URL: `http://127.0.0.1:8765/google-asr?autostart=1[&locale=…]`.
- Isolated Chrome profile: `user-data/browser-worker-profile-classic-*`.
- Anti-throttle flags + EcoQoS opt-out on Windows.

### Element: recognition language
- **Settings** → Web Speech / `asr.browser.recognition_language`.
- Worker UI shows live/final text and WS diagnostics.
- If worker has text but dashboard is empty — issue is ingest/WS, not Chrome recognition.

### Element: advanced Web Speech settings
- **Settings** → “Advanced Web Speech settings” (`asr.browser.*`, `asr.realtime` partial filters).
- Groups: forced final, restart, network reconnect, session rotation, partial filtering.
- Each field has an **`!` help button** with a short description of what it affects.
- **Defaults (0.5.4+):** faster restarts (150 ms), stricter forced-final threshold (8 chars), earlier session prepare cycle (30 s before 3 min max age). See `docs/TECHNICAL_ARCHITECTURE.en.md` §12.
- **Deprecated (ignore in manual config):** `pause_to_finalize_ms` / `finalization_hold_ms`, `hard_max_phrase_ms` / `max_segment_ms` — legacy sync only; use worker **`force_finalization_timeout_ms`** for idle forced-final timing.
- After changes: Save config → **Stop/Start** and reopen worker if needed.

### Element: worker stability (ported from SST)
- Screen Wake Lock while recognition runs.
- Session rotation `max_browser_session_age_ms` (default 180000 ms).
- Network preflight → terminal `recognition_network_unreachable` after repeated network errors.
- Force-finalization for stuck partials.
- **Long-segment flush (0.5.4+):** after a committed final ≥200 characters, worker resets the Web Speech results buffer so the next phrases are not chopped into short finals. See `docs/TECHNICAL_ARCHITECTURE.en.md` §12.

**Not in core:** legacy SST local ASR (`asr.mode: local`), experimental `/google-asr-experimental`. Use the **Local ASR module** (`local_parakeet`) instead of legacy `local`.

---

## 4a. Local ASR module (Parakeet)

### Element: `/local-asr` window
- Separate module UI (like TTS): deps check, model download, CPU/CUDA EP, realtime presets, mic test bench.
- Open from **Modules** or Tauri IPC `local_asr_open_window`.
- Settings live in `user-data/modules/local-asr/config.toml` (project-wide; window can be closed).

### Element: readiness gate
- Live tab shows **Local ASR** mode only when `asr.local_module.ready` (CPU path: ORT + model + warm load).
- `cuda_ready` is an extra badge for NVIDIA CUDA EP; CPU path is enough for Live.
- After changing realtime/VAD settings: **Stop → Start** the Live session.

### Element: Live with Local ASR
- Select `local_parakeet` on Overview → **Start** — no Chrome worker; mic capture is native (cpal).
- Text goes through the same subtitle/translation/overlay path as Web Speech.
- Details: `TECHNICAL_ARCHITECTURE.en.md` §18.

---

## 5. Translation

### 5.1 Main toggles

#### Element: enable translation
- Turns translation pipeline on/off.
- ASR works without translation (source-only flow).

#### Element: translation cache
- **In memory:** skip duplicate provider calls.
- **On disk:** `user-data/translation-cache/` across sessions.
- **Trade-off:** old cache may conflict with changed LLM prompts.

### 5.2 Translation lines (`translation_1`…`translation_5`)

- Up to **5** independent lines: `enabled`, `target_lang`, `provider`, `label`.
- Each enabled line adds dispatcher load.
- Display order — **Subtitles** tab (slot ids).

### 5.3 Providers (13)

`google_translate_v2` (default), `google_cloud_translation_v3`, `google_gas_url`, `google_web`, `azure_translator`, `deepl`, `libretranslate`, `openai`, `openrouter`, `lm_studio`, `ollama`, `public_libretranslate_mirror`, `free_web_translate`.

- OpenAI-compatible helpers: `/api/openai/recommended-models`, `/api/openai/models` (static list).
- Credentials in `translation.provider_settings` — stored locally in `config.toml`.

### 5.4 Translation dispatcher
- Timeout, queue size, max concurrent jobs.
- Per-provider limits (`provider_limits`).
- **Lifecycle:** completed block stays until new phrase finalizes; late translations allowed; stale drop for superseded in-flight jobs only.

### 5.5 Translation results block
- Shows latest translations and provider errors.
- Delayed translation is not always failure (supersession / stale protection).

---

## 6. Subtitle output (Subtitles)

### Element: overlay preset
- `single`, `dual-line`, `stacked`, `compact`.
- Query override: `?preset=…&compact=1`.

### Element: visibility
- Source / translation toggles.
- Max visible translation lines.

### Element: TTL and lifecycle
- `completed_block_ttl_ms`, source/translation TTL.
- Sync flags: keep source while translation visible.
- **Intent:** completed translation stays visible while next phrase is still partial; replacement after new phrase finalizes.

### Element: line order
- Affects dashboard preview, OBS overlay, and OBS CC `first_visible_line` mode.

---

## 7. Subtitle style (Style)

- Built-in and custom presets.
- Base controls: font, size, weight, color, outline, shadow, background, alignment, spacing.
- Effects: `none`, `fade`, `subtle_pop`, `slide_up`, `zoom_in`, `blur_in`, `glow`.
- Per-slot overrides: `source`, `translation_1`…`translation_5`.
- **Shared payload** for dashboard preview and OBS overlay.
- Save config/profile after edits.

---

## 8. UI theme (Theme)

- Dark / light mode.
- Accent palette gradients.
- Affects dashboard chrome only; OBS overlay uses subtitle-style config.

---

## 9. OBS

### Element: overlay URL
- Copied from **OBS** tab (`GET /api/obs/url`).
- Default: `http://127.0.0.1:8765/overlay`.
- Update OBS if bind changes (LAN mode).

### Element: OBS Closed Captions
- WebSocket host/port/password (OBS v5).
- Output mode: source live/final, translation slots, first visible line.
- Timing: partial throttle, min delta, clear after ms, dedupe.
- Debug mirror — text source for CC debugging.

---

## 10. Word replacement

- Find/replace applied **before** translation and display (`TranscriptController`).
- Built-in lists + **stems** (en/ru) and obfuscation normalization (leet, separators, letter repeats).
- Case-insensitive / whole words (CJK uses substring match, not `\b`).
- Twitch chat TTS uses its own `include_builtin_profanity` flag (not dashboard custom pairs).

---

## 11. TTS module

### Element: `/tts` window
- Separate UI: **Speech** and **Twitch** tabs.
- Open from dashboard or Tauri IPC `tts_open_window`.

### Element: Speech
- TTS provider, voice, rate/pitch/volume.
- **Volume:** 0–**150%** (native `amplify` via IPC); slider with live numeric label (`85%`, `150%`).
- **Playback:** **Native** mode (cpal @ 1.0×) or **Sonic** (libsonic tempo stretch); separate WASAPI devices for speech and Twitch.
- Subtitle-driven planner runs in Rust (`speech_pipeline.rs`); manual sample test via `tts_speak_sample`; playback via IPC `tts_play_audio` (no browser HTMLAudio).

### Element: Twitch
- OAuth via system browser; implicit grant + token poll.
- **Up to 5 channels** per connection (channel logins without `#`); status badge `IRC: connected #channel` or `3/5 channels`.
- IRC chat → speech queue (`twitch` channel); emote/link/symbol/lang filters apply **live** without IRC reconnect.
- **Auto-reconnect** on IRC/TLS drop — exponential backoff 1→30 s; OAuth/auth errors do not retry; manual Disconnect stops the loop.
- **Symbols not spoken** field — comma-separated tokens removed from text (empty = all symbols may be read).
- **Advanced:** optional rate/volume overrides with live numeric labels (`1.25×`, `85%`) like the Speech tab; `@mentions` spoken with username (no `@`); with **`strip_links=false`**, URLs stay in speak text.
- Digits in messages (`5`, `100`, `500&100`, ordinals like `5ю`) preserved during emoji/emote stripping; invisible filler chars (U+034F, etc.) stripped before filters.
- **?** help on **Bot nick** — IRC login used for `JOIN` (not a viewer display name); popover clamped to viewport (`popover-position.ts`).
- Crate `voicesub-twitch`; UI `TwitchPanel.svelte`; config `user-data/modules/tts/config.toml`.

### Element: Python sidecar
- `bin/modules/tts/runtime/` — embedded fetcher for Google TTS proxy.
- `/api/tts/python/status` — runtime probe.

---

## 12. Tools & Data

### Element: Runtime Diagnostics
- Phase, worker connected, translation queue, OBS CC state, metrics.
- Log paths: `logs/core.log`, `runtime-events.log`, `session-latest.jsonl`.

### Element: profiles
- CRUD via UI → `user-data/profiles/{name}.toml`.
- Quick switching between streaming setups.

### Element: Export Diagnostics
- ZIP: redacted config, runtime status, session log, core log.
- `GET /api/exports/diagnostics`.

### Element: deep diagnostics (env)
- `VOICESUB_DEEP_DIAGNOSTICS=1` or `logging.full_enabled` in config.
- Per-channel: `VOICESUB_TRACE_SUBTITLE`, `_BROWSER`, `_WS`, `_TTS`, …

---

## 13. Settings

### Element: UI language (EN / RU / JA / KO / ZH)
- Svelte i18n: `src/lib/i18n/locales/*.json` (generated from `scripts/i18n-source/locales/*.js`).
- Saved in `ui.language` → Save config.
- Worker gets `locale` query param on launch.
- Overlay i18n: `bin/overlay/shared/js/i18n/` (regenerate: `npm run i18n:bundle` after editing source locales).
- **Details:** `TECHNICAL_ARCHITECTURE.en.md` §24.

### Element: SST config.json import
- Migrates to `config.toml`, `config_version` → 8.
- `local` / experimental → `browser_google` + import hint.
- `local_parakeet` is **preserved** (runtime still requires module `ready` before Live use).

### Element: layout
- `standard` vs `compact` — affects Tauri window size.

---

## 14. Help

Built-in topics: overview, recognition, translation, subtitles/style, OBS, tools.

---

## 15. Privacy and local-first

- **Local-first:** default `127.0.0.1`; LAN only with `VOICESUB_ALLOW_LAN=1`.
- API keys and Twitch tokens — local disk only.
- Diagnostics export — redacted secrets.
- Chrome worker — isolated profile, no sync.

---

## 16. Glossary

| Term | Meaning |
| --- | --- |
| **partial** | In-progress recognized text |
| **final** | Finalized phrase segment |
| **translation slot** | Line `translation_1`…`translation_5` |
| **overlay** | Vanilla `/overlay` page for OBS |
| **browser worker** | Chrome window running Web Speech |
| **completed block** | Final subtitle until next phrase finalizes |
| **TTS module** | Sidecar `/tts` + Rust service |
| **Local ASR** | Sidecar `/local-asr` + Parakeet ONNX (`local_parakeet`) |

---

## 17. Archived features (not in core 0.5.0)

| SST feature | VoiceSub status |
| --- | --- |
| Legacy local ASR (`asr.mode: local`) | Removed from core; SST import maps `local` → `browser_google`. Successor: Local ASR module (`local_parakeet`) |
| Experimental browser | `legacy/experimental-browser/` — routes removed |
| PyInstaller bootstrap | Replaced by Tauri NSIS installer |
| Splash startup profiles | None — single `VoiceSub.exe` |

For browser/translation/subtitle parity, see golden tests in `tests/golden/`.
