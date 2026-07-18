# VoiceSub

**Live translated subtitles for streamers — local-first, privacy-first, OBS-ready.**

[![Version](https://img.shields.io/badge/version-0.6.0-blue.svg)](./docs/CHANGELOG.en.md)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-lightgrey.svg)](#system-requirements)
[![Stack](https://img.shields.io/badge/stack-Rust%20%2B%20Tauri%20%2B%20Svelte-orange.svg)](#development)
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

---

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

---

## System requirements

- Windows 10 or 11 (x64)
- **Microsoft Edge WebView2 Runtime** (usually preinstalled on Windows 11; the NSIS installer can bootstrap it on Windows 10)
- **Google Chrome** — only for the Web Speech worker (not needed for Local ASR alone)
- Microphone access
- Internet — optional for cloud translation providers; also used for first-time Local ASR model / ORT downloads

No Python, Node.js, or CUDA in the core installer. CUDA is an optional Local ASR download.

---

## Quick start

1. Install from `VoiceSub_0.6.0_x64-setup.exe` (or the latest build in your release folder).
2. Launch **VoiceSub.exe** — the dashboard opens at `http://127.0.0.1:8765/`.
3. In OBS, add a **Browser Source** → `http://127.0.0.1:8765/overlay`.
4. Configure translation and subtitle style if needed, then click **Start**.
5. Choose recognition:
   - **Web Speech** — keep the browser worker window open and visible (mic permission is granted there).
   - **Local ASR** — **Modules → Local ASR**, finish setup until ready, select Local ASR on Live, then Start.

Step-by-step UI guide: [Wiki (EN)](./docs/WIKI.en.md) · [Wiki (RU)](./docs/WIKI.ru.md)

---

## Local URLs

| URL | Purpose |
| --- | --- |
| `http://127.0.0.1:8765/` | Dashboard |
| `http://127.0.0.1:8765/overlay` | OBS Browser Source |
| `http://127.0.0.1:8765/google-asr?autostart=1` | Browser Speech worker |
| `http://127.0.0.1:8765/tts` | TTS module |
| `http://127.0.0.1:8765/local-asr` | Local ASR module |

Overlay query examples: `?preset=single` · `?compact=1` · `?profile=default`

---

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

---

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

---

## Documentation

- [Wiki (EN)](./docs/WIKI.en.md) / [Wiki (RU)](./docs/WIKI.ru.md) — user guide
- [Technical Architecture (EN)](./docs/TECHNICAL_ARCHITECTURE.en.md) / [(RU)](./docs/TECHNICAL_ARCHITECTURE.md)
- [Changelog (EN)](./docs/CHANGELOG.en.md) / [(RU)](./docs/CHANGELOG.md) — [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)

---

## Contributing

Pull requests are welcome. For larger changes, open an issue first.

```powershell
cargo test --workspace
npm run build
npm run test:frontend
```

---

## License

[MIT](./LICENSE) © 2026 Kiriuru

---

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
