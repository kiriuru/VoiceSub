# VoiceSub

**Live translated subtitles for streamers — local-first, privacy-first, OBS-ready.**

[![Version](https://img.shields.io/badge/version-0.6.0-blue.svg)](./docs/CHANGELOG.en.md)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-lightgrey.svg)](#system-requirements)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](./LICENSE)
[![Changelog](https://img.shields.io/badge/changelog-Keep%20a%20Changelog-E05735.svg)](./docs/CHANGELOG.en.md)

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="./README.ru.md">Русский</a> ·
  <a href="./docs/WIKI.en.md">Wiki</a> ·
  <a href="./docs/TECHNICAL_ARCHITECTURE.en.md">Architecture</a> ·
  <a href="./docs/CHANGELOG.en.md">Changelog</a>
</p>

VoiceSub is a Windows desktop app that turns speech into real-time subtitles with optional translation. Recognition runs through **Google Chrome Web Speech** or optional offline **Local ASR** (Parakeet / ONNX). Everything stays on your machine — default bind `127.0.0.1:8765`, no cloud backend, no accounts.

Successor to SST Desktop `0.4.4`. First VoiceSub release: **`0.5.0`**. Current line: **`0.6.0`**.

<p align="center">
  <img src="./Images/Live_window.jpg" alt="VoiceSub Live tab" width="860">
  <br>
  <em>Live — Start/Stop, recognition status, transcript, and subtitle preview</em>
</p>

## Table of contents

- [Features](#features)
- [Screenshots](#screenshots)
- [System requirements](#system-requirements)
- [Quick start](#quick-start)
- [Local URLs](#local-urls)
- [Data paths](#data-paths)
- [Troubleshooting](#troubleshooting)
- [Documentation](#documentation)
- [Contributing](#contributing)
- [License](#license)

## Features

| Area | What you get |
| --- | --- |
| **Speech** | Google Chrome Web Speech worker, or offline Local ASR (Parakeet / ONNX, CPU or CUDA) |
| **Translation** | 17 providers (incl. Baidu / Youdao / Tencent / Caiyun), up to 5 translation lines |
| **OBS** | Browser Source overlay + optional Closed Captions (WebSocket) |
| **Style** | Animated presets, per-slot styling, theme palette |
| **TTS** | Native / Sonic playback; subtitle speech + Twitch chat TTS (up to 5 channels) |
| **Local ASR** | Setup wizard at `/local-asr`; Live mode `local_parakeet` when ready |
| **Ops** | Diagnostics ZIP export; UI locales en / ru / ja / ko / zh |

Compact phone-style layout is available for secondary monitors.

## Screenshots

<table>
  <tr>
    <td align="center" width="50%">
      <img src="./Images/Translation_window.jpg" alt="Translation tab" width="420"><br>
      <strong>Translation</strong><br>
      <sub>Providers, cache, and up to 5 translation lines</sub>
    </td>
    <td align="center" width="50%">
      <img src="./Images/Subtitles_window.jpg" alt="Subtitles tab" width="420"><br>
      <strong>Subtitles</strong><br>
      <sub>Overlay preset, visibility, order, and TTL</sub>
    </td>
  </tr>
  <tr>
    <td align="center">
      <img src="./Images/Subtitle_Style_window.jpg" alt="Subtitle Style tab" width="420"><br>
      <strong>Subtitle Style</strong><br>
      <sub>Fonts, colors, effects, and per-slot styles</sub>
    </td>
    <td align="center">
      <img src="./Images/OBS_window.jpg" alt="OBS tab" width="420"><br>
      <strong>OBS</strong><br>
      <sub>Overlay URL and Closed Captions</sub>
    </td>
  </tr>
  <tr>
    <td align="center">
      <img src="./Images/modules_window.jpg" alt="Modules tab" width="420"><br>
      <strong>Modules</strong><br>
      <sub>Open sidecar TTS and Local ASR windows</sub>
    </td>
    <td align="center">
      <img src="./Images/Web_Speech_Window.jpg" alt="Web Speech settings" width="420"><br>
      <strong>Web Speech</strong><br>
      <sub>Chrome worker language and advanced recognition options</sub>
    </td>
  </tr>
  <tr>
    <td align="center">
      <img src="./Images/Local_ASR_window.jpg" alt="Local ASR module" width="420"><br>
      <strong>Local ASR</strong><br>
      <sub>Offline Parakeet / ONNX setup (CPU or CUDA)</sub>
    </td>
    <td align="center">
      <img src="./Images/TTS_window.jpg" alt="TTS module" width="420"><br>
      <strong>TTS</strong><br>
      <sub>Subtitle speech and Twitch chat TTS</sub>
    </td>
  </tr>
  <tr>
    <td align="center">
      <img src="./Images/UI_Theme_window.jpg" alt="UI Theme tab" width="420"><br>
      <strong>UI Theme</strong><br>
      <sub>Dark/light mode and accent palette</sub>
    </td>
    <td align="center">
      <img src="./Images/Settings_window.jpg" alt="Settings tab" width="420"><br>
      <strong>Settings</strong><br>
      <sub>UI language, layout, and SST config import</sub>
    </td>
  </tr>
</table>

More UI walkthroughs (Word Replace, Tools & Data, Help, Local ASR components): [Wiki](./docs/WIKI.en.md).

## System requirements

- Windows 10 or 11 (x64)
- **Microsoft Edge WebView2 Runtime** (usually preinstalled on Windows 11; the NSIS installer can bootstrap it on Windows 10)
- **Google Chrome** — only for the Web Speech worker (not needed for Local ASR alone)
- Microphone access
- Internet — optional for cloud translation providers; also used for first-time Local ASR model / ORT downloads

No Python, Node.js, or CUDA in the core installer. CUDA is an optional Local ASR download.

## Quick start

1. Install from `VoiceSub_0.6.0_x64-setup.exe` (or the latest build in your release folder).
2. Launch **VoiceSub.exe** — the dashboard opens at `http://127.0.0.1:8765/`.
3. In OBS, add a **Browser Source** → `http://127.0.0.1:8765/overlay`.
4. Configure translation and subtitle style if needed, then click **Start**.
5. Choose recognition:
   - **Web Speech** — keep the browser worker window open and visible (mic permission is granted there).
   - **Local ASR** — **Modules → Local ASR**, finish setup until ready, select Local ASR on Live, then Start.

Step-by-step UI guide: [Wiki (EN)](./docs/WIKI.en.md) · [Wiki (RU)](./docs/WIKI.ru.md)

## Local URLs

| URL | Purpose |
| --- | --- |
| `http://127.0.0.1:8765/` | Dashboard |
| `http://127.0.0.1:8765/overlay` | OBS Browser Source |
| `http://127.0.0.1:8765/google-asr?autostart=1` | Browser Speech worker |
| `http://127.0.0.1:8765/tts` | TTS module |
| `http://127.0.0.1:8765/local-asr` | Local ASR module |

Overlay query examples: `?preset=single` · `?compact=1` · `?profile=default`

## Data paths

| Path | Contents |
| --- | --- |
| `user-data/config.toml` | Main settings |
| `user-data/profiles/` | Named profiles |
| `user-data/modules/tts/` | TTS settings |
| `user-data/modules/local-asr/` | Local ASR config, models, ORT / CUDA runtime |
| `user-data/translation-cache/` | Translation cache |
| `logs/` | `core.log`, `runtime-events.log`, `session-latest.jsonl` |
| `bin/fonts/` | Subtitle fonts |

SST `config.json` can be imported on first run or from settings. Legacy `local` / experimental modes map to `browser_google`; `local_parakeet` is preserved. Details: [Architecture §7](./docs/TECHNICAL_ARCHITECTURE.en.md).

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| No subtitles | **Start** pressed; worker open (Web Speech) **or** Local ASR ready + mic selected |
| Source text, no translation | Translation on; at least one line active; provider credentials |
| Empty OBS | Browser Source URL is `/overlay`; visibility on Subtitles tab; reload source after updates |
| Text stuck after TTL / Stop | Update build; reload Browser Source |
| Port in use | Free `8765` or change bind (dev builds) |
| Local ASR missing on Live | Modules → Local ASR: finish wizard until `ready` |

Full guide: [Wiki → Troubleshooting](./docs/WIKI.en.md).

## Documentation

- [Wiki (EN)](./docs/WIKI.en.md) / [Wiki (RU)](./docs/WIKI.ru.md) — user guide
- [Technical Architecture (EN)](./docs/TECHNICAL_ARCHITECTURE.en.md) / [(RU)](./docs/TECHNICAL_ARCHITECTURE.md)
- [Changelog (EN)](./docs/CHANGELOG.en.md) / [(RU)](./docs/CHANGELOG.md) — [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)

## Contributing

Pull requests are welcome. For larger changes, open an issue first.

```powershell
cargo test --workspace
npm run build
npm run test:frontend
```

<details>
<summary><strong>Developers — stack and build</strong></summary>

### Stack

| Layer | Tech |
| --- | --- |
| Core | Rust workspace (`crates/voicesub-*`) + Axum HTTP/WS |
| Shell | Tauri 2 → `VoiceSub.exe` (NSIS) |
| Dashboard | Svelte 5 + Vite → `bin/dashboard/` |
| Worker | Svelte 5 → `bin/worker/` |
| Overlay | Vanilla HTML/JS → `bin/overlay/` |
| TTS | Svelte + Rust service + embedded Python sidecar |
| Local ASR | Svelte + `voicesub-asr-local` + ONNX Runtime (lazy download) |

Node.js is **build-time only** — not shipped in the installer.

### Build from source

```powershell
npm install
npm run build          # dashboard + worker + TTS + Local ASR
npm run i18n:export    # scripts/i18n-source → locale JSON
npm run i18n:bundle    # overlay locales bundle
cargo test --workspace
build-release-msi.bat  # → NSIS setup.exe in release_root
```

Tauri `beforeBuildCommand`: `npm run build`. Bundled resources: `bin/dashboard`, `overlay`, `worker`, `tts`, `local-asr`, `fonts`, `modules`.

### Key crates

`voicesub-runtime` · `voicesub-subtitle` · `voicesub-translation` · `voicesub-browser` · `voicesub-ws` · `voicesub-tts` · `voicesub-asr-local` · `voicesub-partial-emit` · `voicesub-obs`

`src-tauri/` is a thin IPC shell — no domain logic.

Version source: `voicesub-types::PROJECT_VERSION` = **`0.6.0`**.

Full reference: [Technical Architecture](./docs/TECHNICAL_ARCHITECTURE.en.md).

</details>

## License

[MIT](./LICENSE) © 2026 Kiriuru
