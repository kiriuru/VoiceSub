# VoiceSub Wiki

Operational guide for the VoiceSub **`0.6.0`** UI — what each control is for, how it behaves, and what usually goes wrong.

<p align="center">
  <a href="../README.md">README</a> ·
  <a href="./WIKI.ru.md">Русский</a> ·
  <a href="./TECHNICAL_ARCHITECTURE.en.md">Architecture</a> ·
  <a href="./CHANGELOG.en.md">Changelog</a>
</p>

> [!TIP]
> On GitHub, open **Outline** (list icon in the file header) for an auto-generated sidebar from headings. Use the jump bar and table of contents below for in-page links.

## Jump bar

<p align="center">
  <a href="#quick-start"><code>Start</code></a> ·
  <a href="#troubleshooting"><code>Fix</code></a> ·
  <a href="#dashboard-tabs"><code>Tabs</code></a> ·
  <a href="#browser-speech-web-speech"><code>Web Speech</code></a> ·
  <a href="#local-asr"><code>Local ASR</code></a> ·
  <a href="#translation"><code>Translation</code></a> ·
  <a href="#subtitles"><code>Subtitles</code></a> ·
  <a href="#obs"><code>OBS</code></a> ·
  <a href="#tts-module"><code>TTS</code></a> ·
  <a href="#tools-and-data"><code>Tools</code></a> ·
  <a href="#settings"><code>Settings</code></a> ·
  <a href="#glossary"><code>Glossary</code></a>
</p>

## Table of contents

<details open>
<summary><strong>Expand / collapse contents</strong></summary>

1. [About](#about)
2. [Quick start](#quick-start)
3. [Troubleshooting](#troubleshooting)
4. [Dashboard tabs](#dashboard-tabs)
5. [Browser Speech (Web Speech)](#browser-speech-web-speech)
6. [Local ASR](#local-asr)
7. [Translation](#translation)
8. [Subtitles](#subtitles)
9. [Subtitle style](#subtitle-style)
10. [UI theme](#ui-theme)
11. [OBS](#obs)
12. [Word replacement](#word-replacement)
13. [TTS module](#tts-module)
14. [Tools and data](#tools-and-data)
15. [Settings](#settings)
16. [Help](#help)
17. [Privacy and local-first](#privacy-and-local-first)
18. [Glossary](#glossary)
19. [Archived features](#archived-features)

</details>

---

## About

<a id="about-top"></a>

VoiceSub is the active `0.6.0` line (Rust + Tauri + Svelte). Baseline first release: `0.5.0`. SST Desktop `0.4.4` is a frozen predecessor — settings import works, but legacy local ASR and experimental browser modes are not started in core.

> [!IMPORTANT]
> Overlay URL is `http://127.0.0.1:8765/overlay`. If you upgraded from SST, update the OBS Browser Source manually.

### System requirements

| Requirement | Notes |
| --- | --- |
| Windows 10/11 x64 | Required |
| WebView2 Runtime | Required for `VoiceSub.exe`, `/tts`, `/local-asr`. Usually present on Windows 11; installer may bootstrap on Windows 10 |
| Google Chrome | Only for Web Speech worker (`/google-asr`). Not needed for Local ASR alone |
| Microphone | Chrome worker **or** Local ASR native capture |
| Internet | Optional for cloud translation; also used for first-time Local ASR model / ORT downloads |

### Install and update (NSIS)

- Installer: `VoiceSub_0.6.0_x64-setup.exe` → `VoiceSub.exe` + bundled assets (dashboard, overlay, worker, tts, local-asr).
- No Python/Node in runtime; WebView2 via Tauri `downloadBootstrapper` when missing.
- **Update:** close app → run new `setup.exe` over existing → `user-data/` and `logs/` persist.
- **Update check:** dashboard polls GitHub Releases (`POST /api/updates/check`). Banner shows when a newer tag exists; **Download** opens the release page. Config: `user-data/config.toml` → `[updates]`.
- Developers: `build-release-msi.bat` → `build-release.ps1` → release folder under `F:\AI\VoiceSub - release\v{version}\`.

### Local URLs

| URL | Purpose |
| --- | --- |
| `/` | Svelte dashboard |
| `/overlay` | OBS Browser Source |
| `/google-asr` | Browser Speech worker |
| `/tts` | TTS module UI |
| `/local-asr` | Local ASR module UI |

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Quick start

<p align="center">
  <img src="../Images/Live_window.jpg" alt="Live tab" width="820"><br>
  <em><strong>Live</strong> — Start/Stop, recognition status, transcript, subtitle preview</em>
</p>

### First run

1. Launch **VoiceSub.exe**.
2. Dashboard opens in the Tauri main window (`http://127.0.0.1:8765/`).
3. Add OBS Browser Source: `http://127.0.0.1:8765/overlay`.
4. Set UI language (**Settings**) and translation (**Translation**) if needed.
5. Click **Start** — Chrome opens `/google-asr?autostart=1` (Web Speech) **or** Local ASR starts in-process when `local_parakeet` is selected and ready.
6. Grant microphone permission in Chrome (Web Speech) **or** pick a mic in the Local ASR module, then speak.

### Runtime bar (Start / Stop)

| Action | Behavior |
| --- | --- |
| **Start** | `POST /api/runtime/start` — worker, translation, OBS CC, ASR ingest |
| **Stop** | Stops worker (kills Chrome tree), resets subtitle state |

> [!NOTE]
> Start sends the current config snapshot, including unsaved edits since the last Save.

### Subtitle preview

Top **Subtitle Output Preview** shows placeholders before Start and the live payload after. Use it to calibrate style without ASR. An empty post-save `overlay_update` does not clear the preview. Details: [Architecture §21](./TECHNICAL_ARCHITECTURE.en.md).

### Compact layout

Switches the Tauri window (~390×844) with a **Live** pane + settings tabs. Toggle via the layout button or command palette (`Ctrl+K`). IPC: `set_dashboard_layout`.

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Troubleshooting

> [!TIP]
> Work top-down: runtime Start → recognition path → translation → overlay URL.

### No text at all

- [ ] Runtime **Start**ed?
- [ ] **Web Speech:** Chrome `/google-asr` open and **visible**? Mic allowed in **Chrome**?
- [ ] **Local ASR:** mode `local_parakeet` selected and module `ready`? Mic selected in `/local-asr`?
- [ ] **Tools & Data** → diagnostics: `browser_worker_connected` or Local ASR status

### Source text but no translation

- [ ] **Translation** tab → translation enabled
- [ ] At least one `translation_N` line with `enabled`
- [ ] Check translation results / diagnostics for provider errors

### OBS shows nothing

- [ ] Browser Source URL is `/overlay` (not dashboard `/`)
- [ ] **Subtitles** → source/translation visibility enabled
- [ ] TTL not too aggressive (text may flash and vanish)
- [ ] After WS disconnect, overlay keeps last frame (stale-guard + 1–10 s backoff) — expected
- [ ] Text stuck after TTL/Stop → update the app and reload Browser Source

<details>
<summary><strong>Worker keeps dying</strong></summary>

- Check network (Web Speech uses Google endpoints).
- Enable `VOICESUB_TRACE_BROWSER=1` → `logs/browser-trace.jsonl`.
- Recovery: **Stop** → **Start**, or relaunch worker from Tools.

</details>

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Dashboard tabs

| Tab | Purpose | Section |
| --- | --- | --- |
| **Translation** | Providers, lines, cache, dispatcher limits | [Translation](#translation) |
| **Subtitles** | Overlay preset, visibility, order, TTL | [Subtitles](#subtitles) |
| **Style** | Fonts, colors, effects, slot styles | [Subtitle style](#subtitle-style) |
| **UI Theme** | Dark/light, accent palette | [UI theme](#ui-theme) |
| **OBS** | Overlay URL, Closed Captions | [OBS](#obs) |
| **Word Replace** | Text replacement before translation | [Word replacement](#word-replacement) |
| **Tools & Data** | Profiles, diagnostics, ZIP export | [Tools and data](#tools-and-data) |
| **Settings** | Language, layout, SST import, Web Speech advanced | [Settings](#settings) |
| **Help** | Built-in topics | [Help](#help) |

**Command palette** (header search / `Ctrl+K`): quick navigation, Start/Stop, Save, export diagnostics.

<table>
  <tr>
    <td align="center" width="33%">
      <img src="../Images/Translation_window.jpg" alt="Translation" width="280"><br>
      <sub><a href="#translation">Translation</a></sub>
    </td>
    <td align="center" width="33%">
      <img src="../Images/Subtitles_window.jpg" alt="Subtitles" width="280"><br>
      <sub><a href="#subtitles">Subtitles</a></sub>
    </td>
    <td align="center" width="33%">
      <img src="../Images/Subtitle_Style_window.jpg" alt="Style" width="280"><br>
      <sub><a href="#subtitle-style">Style</a></sub>
    </td>
  </tr>
  <tr>
    <td align="center">
      <img src="../Images/UI_Theme_window.jpg" alt="UI Theme" width="280"><br>
      <sub><a href="#ui-theme">UI Theme</a></sub>
    </td>
    <td align="center">
      <img src="../Images/OBS_window.jpg" alt="OBS" width="280"><br>
      <sub><a href="#obs">OBS</a></sub>
    </td>
    <td align="center">
      <img src="../Images/Word_replacement_window.jpg" alt="Word Replace" width="280"><br>
      <sub><a href="#word-replacement">Word Replace</a></sub>
    </td>
  </tr>
</table>

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Browser Speech (Web Speech)

<p align="center">
  <img src="../Images/Web_Speech_Window.jpg" alt="Web Speech settings" width="820"><br>
  <em><strong>Web Speech</strong> — recognition language and advanced worker options</em>
</p>

### Mode

- Production mode: **`browser_google`** — Web Speech in a separate Chrome window.
- Microphone is selected **in Chrome** (`getUserMedia`), not in the dashboard.
- `/api/devices/audio-inputs` returns empty — by design.

### Browser worker window

- Separate window with a **visible address bar** (no app mode, no hidden tab).
- URL: `http://127.0.0.1:8765/google-asr?autostart=1[&locale=…]`.
- Isolated Chrome profile: `user-data/browser-worker-profile-classic-*`.
- Anti-throttle flags + EcoQoS opt-out on Windows.

### Recognition language

**Settings** → Web Speech / `asr.browser.recognition_language`. Worker UI shows live/final text and WS diagnostics. If the worker has text but the dashboard is empty — the issue is ingest/WS, not Chrome recognition.

<details>
<summary><strong>Advanced Web Speech settings</strong></summary>

- Location: **Settings** → “Advanced Web Speech settings” (`asr.browser.*`, `asr.realtime` partial filters).
- Groups: forced final, restart, network reconnect, session rotation, partial filtering.
- Each field has an **`!` help button**.
- **Defaults (0.5.4+):** faster restarts (150 ms), stricter forced-final threshold (8 chars), earlier session prepare (30 s before 3 min max age). See [Architecture §12](./TECHNICAL_ARCHITECTURE.en.md).
- **Deprecated (ignore in manual config):** `pause_to_finalize_ms` / `finalization_hold_ms`, `hard_max_phrase_ms` / `max_segment_ms` — use worker **`force_finalization_timeout_ms`** for idle forced-final timing.
- After changes: Save → **Stop/Start** and reopen the worker if needed.

</details>

<details>
<summary><strong>Worker stability</strong></summary>

- Screen Wake Lock while recognition runs.
- Session rotation `max_browser_session_age_ms` (default 180000 ms).
- Network preflight → terminal `recognition_network_unreachable` after repeated network errors.
- Force-finalization for stuck partials.
- **Long-segment flush (0.5.4+):** after a committed final ≥200 characters, worker resets the Web Speech results buffer. See [Architecture §12](./TECHNICAL_ARCHITECTURE.en.md).

</details>

> [!WARNING]
> Legacy SST `asr.mode: local` and experimental `/google-asr-experimental` are **not** in core. Use the [Local ASR](#local-asr) module (`local_parakeet`) instead.

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Local ASR

<p align="center">
  <img src="../Images/modules_window.jpg" alt="Modules" width="400">
  &nbsp;
  <img src="../Images/Local_ASR_window.jpg" alt="Local ASR module" width="400"><br>
  <em><strong>Modules</strong> / <strong>Local ASR</strong> — open sidecar and finish setup until ready</em>
</p>

<p align="center">
  <img src="../Images/Local_ASR_components_window.jpg" alt="Local ASR components" width="720"><br>
  <em><strong>Components</strong> — ORT, Parakeet model, optional CUDA</em>
</p>

### Module window (`/local-asr`)

Separate UI (like TTS): deps check, model download, CPU/CUDA EP, realtime presets, mic test bench. Open from **Modules** or Tauri IPC `local_asr_open_window`. Settings: `user-data/modules/local-asr/config.toml` (window can be closed).

### Readiness gate

- Live shows **Local ASR** only when `asr.local_module.ready` (CPU path: ORT + model + warm load).
- `cuda_ready` is an extra badge for NVIDIA CUDA EP; CPU is enough for Live.
- After changing realtime/VAD settings: **Stop → Start** the Live session.

### Live with Local ASR

Select `local_parakeet` on Live → **Start** — no Chrome worker; mic capture is native (cpal). Text uses the same subtitle/translation/overlay path as Web Speech. Details: [Architecture §18](./TECHNICAL_ARCHITECTURE.en.md).

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Translation

<p align="center">
  <img src="../Images/Translation_window.jpg" alt="Translation tab" width="820"><br>
  <em><strong>Translation</strong> — enable pipeline, providers, and up to 5 lines</em>
</p>

### Main toggles

| Control | Behavior |
| --- | --- |
| Enable translation | Turns the translation pipeline on/off. ASR works without it (source-only). |
| Cache (memory) | Skip duplicate provider calls |
| Cache (disk) | `user-data/translation-cache/` across sessions |

> [!NOTE]
> Old disk cache may conflict with changed LLM prompts.

### Translation lines (`translation_1`…`translation_5`)

Up to **5** independent lines: `enabled`, `target_lang`, `provider`, `label`. Each enabled line adds dispatcher load. Display order is set on the **Subtitles** tab.

### Providers (17)

`google_translate_v2` (default), `google_cloud_translation_v3`, `google_gas_url`, `google_web`, `azure_translator`, `deepl`, `libretranslate`, `openai`, `openrouter`, `lm_studio`, `ollama`, `public_libretranslate_mirror`, `free_web_translate`, `baidu_translate`, `youdao_translate`, `tencent_tmt`, `caiyun_translator`.

- OpenAI-compatible helpers: `/api/openai/recommended-models`, `/api/openai/models`.
- Credentials in `translation.provider_settings` — local `config.toml` only.

### Dispatcher and results

- Timeout, queue size, max concurrent jobs; per-provider `provider_limits`.
- **Lifecycle:** completed block stays until a new phrase finalizes; late translations allowed; stale drop only for superseded in-flight jobs.
- Results block shows latest translations and provider errors. Delay is not always failure (supersession / stale protection).

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Subtitles

<p align="center">
  <img src="../Images/Subtitles_window.jpg" alt="Subtitles tab" width="820"><br>
  <em><strong>Subtitles</strong> — overlay preset, visibility, order, TTL</em>
</p>

| Topic | Details |
| --- | --- |
| Overlay preset | `single`, `dual-line`, `stacked`, `compact`; query override `?preset=…&compact=1` |
| Visibility | Source / translation toggles; max visible translation lines |
| TTL / lifecycle | `completed_block_ttl_ms`, source/translation TTL; keep source while translation visible |
| Line order | Affects dashboard preview, OBS overlay, and OBS CC `first_visible_line` |

> [!IMPORTANT]
> Completed translation stays visible while the next phrase is still partial; replacement happens after the new phrase finalizes.

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Subtitle style

<p align="center">
  <img src="../Images/Subtitle_Style_window.jpg" alt="Subtitle Style tab" width="820"><br>
  <em><strong>Subtitle Style</strong> — fonts, colors, effects, per-slot overrides</em>
</p>

- Built-in and custom presets.
- Base controls: font, size, weight, color, outline, shadow, background, alignment, spacing.
- Effects: `none`, `fade`, `subtle_pop`, `slide_up`, `zoom_in`, `blur_in`, `glow`.
- Per-slot overrides: `source`, `translation_1`…`translation_5`.
- **Shared payload** for dashboard preview and OBS overlay — Save config/profile after edits.

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## UI theme

<p align="center">
  <img src="../Images/UI_Theme_window.jpg" alt="UI Theme tab" width="820"><br>
  <em><strong>UI Theme</strong> — dark/light mode and accent palette</em>
</p>

Affects **dashboard chrome only**. OBS overlay uses subtitle-style config, not the UI theme.

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## OBS

<p align="center">
  <img src="../Images/OBS_window.jpg" alt="OBS tab" width="820"><br>
  <em><strong>OBS</strong> — overlay URL and Closed Captions</em>
</p>

### Overlay URL

Copied from the **OBS** tab (`GET /api/obs/url`). Default: `http://127.0.0.1:8765/overlay`. Update OBS if bind changes (LAN mode).

### Closed Captions

- WebSocket host/port/password (OBS v5).
- Output mode: source live/final, translation slots, first visible line.
- Timing: partial throttle, min delta, clear after ms, dedupe.
- Debug mirror — text source for CC debugging.

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Word replacement

<p align="center">
  <img src="../Images/Word_replacement_window.jpg" alt="Word Replace tab" width="820"><br>
  <em><strong>Word Replace</strong> — find/replace before translation and display</em>
</p>

- Applied **before** translation and display (`TranscriptController`).
- Built-in lists + **stems** (en/ru) and obfuscation normalization (leet, separators, letter repeats).
- Case-insensitive / whole words (CJK uses substring match, not `\b`).
- Twitch chat TTS uses its own `include_builtin_profanity` flag (not dashboard custom pairs).

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## TTS module

<p align="center">
  <img src="../Images/TTS_window.jpg" alt="TTS module" width="820"><br>
  <em><strong>TTS</strong> — Speech and Twitch tabs in a sidecar window</em>
</p>

Open from **Modules** or Tauri IPC `tts_open_window`. Config: `user-data/modules/tts/config.toml`.

### Speech

- Provider, voice, rate/pitch/volume.
- **Volume:** 0–**150%** (native `amplify`); slider with live label (`85%`, `150%`).
- **Playback:** **Native** (cpal @ 1.0×) or **Sonic** (libsonic tempo stretch); separate WASAPI devices for speech and Twitch.
- Subtitle-driven planner in Rust; sample test via `tts_speak_sample`; playback via in-process `PlaybackHub` (no browser HTMLAudio).

<details>
<summary><strong>Twitch chat TTS</strong></summary>

- OAuth via system browser; implicit grant + token poll.
- **Up to 5 channels** per connection (logins without `#`); badge `IRC: connected #channel` or `3/5 channels`.
- IRC chat → speech queue; emote/link/symbol/lang filters apply **live** without reconnect.
- **Auto-reconnect** on IRC/TLS drop — backoff 1→30 s; OAuth/auth errors do not retry; manual Disconnect stops the loop.
- **Symbols not spoken** — comma-separated tokens removed from text.
- **Advanced:** rate/volume overrides; `@mentions` spoken without `@`; `strip_links=false` keeps URLs.
- Digits preserved during emoji/emote stripping; invisible filler chars stripped before filters.
- **?** on **Bot nick** — IRC login used for `JOIN` (not a viewer display name).

</details>

### Python sidecar

`bin/modules/tts/runtime/` — embedded fetcher for Google TTS proxy. Probe: `/api/tts/python/status`.

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Tools and data

<p align="center">
  <img src="../Images/Tools%26Data_window.jpg" alt="Tools and Data tab" width="820"><br>
  <em><strong>Tools & Data</strong> — profiles, runtime diagnostics, ZIP export</em>
</p>

| Feature | Details |
| --- | --- |
| Runtime diagnostics | Phase, worker connected, translation queue, OBS CC, metrics |
| Logs | `logs/core.log`, `runtime-events.log`, `session-latest.jsonl` |
| Profiles | CRUD → `user-data/profiles/{name}.toml` |
| Export diagnostics | ZIP (redacted config + logs) via `GET /api/exports/diagnostics` |

<details>
<summary><strong>Deep diagnostics (env)</strong></summary>

- `VOICESUB_DEEP_DIAGNOSTICS=1` or `logging.full_enabled` in config.
- Per-channel: `VOICESUB_TRACE_SUBTITLE`, `_BROWSER`, `_WS`, `_TTS`, …

</details>

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Settings

<p align="center">
  <img src="../Images/Settings_window.jpg" alt="Settings tab" width="820"><br>
  <em><strong>Settings</strong> — language, layout, SST import, Web Speech advanced</em>
</p>

### UI language (EN / RU / JA / KO / ZH)

Saved in `ui.language` → Save config. Worker gets `locale` on launch. Overlay i18n: regenerate with `npm run i18n:bundle`. Details: [Architecture §24](./TECHNICAL_ARCHITECTURE.en.md).

### SST `config.json` import

Migrates to `config.toml`, `config_version` → 8. Legacy `local` / experimental → `browser_google`. `local_parakeet` is **preserved** (Live still requires module `ready`).

### Layout

`standard` vs `compact` — Tauri window size.

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Help

<p align="center">
  <img src="../Images/Help_window.jpg" alt="Help tab" width="820"><br>
  <em><strong>Help</strong> — built-in topics inside the app</em>
</p>

Topics: overview, recognition, translation, subtitles/style, OBS, tools.

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Privacy and local-first

- Default bind `127.0.0.1`; LAN only with `VOICESUB_ALLOW_LAN=1`.
- API keys and Twitch tokens stay on local disk.
- Diagnostics export redacts secrets.
- Chrome worker uses an isolated profile (no sync).

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Glossary

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

<p align="right"><a href="#jump-bar">↑ Jump bar</a> · <a href="#table-of-contents">↑ Contents</a></p>

---

## Archived features

| SST feature | VoiceSub status |
| --- | --- |
| Legacy local ASR (`asr.mode: local`) | Removed from core; SST import → `browser_google`. Successor: Local ASR module |
| Experimental browser | Routes removed (`legacy/experimental-browser/`) |
| PyInstaller bootstrap | Replaced by Tauri NSIS installer |
| Splash startup profiles | None — single `VoiceSub.exe` |

For browser/translation/subtitle parity, see golden tests in `tests/golden/`.

---

<p align="center">
  <a href="#jump-bar">↑ Top</a> ·
  <a href="../README.md">README</a> ·
  <a href="./WIKI.ru.md">Русский</a> ·
  <a href="./TECHNICAL_ARCHITECTURE.en.md">Architecture</a>
</p>
