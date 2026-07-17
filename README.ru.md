# VoiceSub

**Живые переводимые субтитры для стримеров — локально, privacy-first, готово для OBS.**

[![Version](https://img.shields.io/badge/version-0.6.0-blue.svg)](./docs/CHANGELOG.md)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-lightgrey.svg)](#системные-требования)
[![Stack](https://img.shields.io/badge/stack-Rust%20%2B%20Tauri%20%2B%20Svelte-orange.svg)](#contributing)
[![Changelog](https://img.shields.io/badge/changelog-Keep%20a%20Changelog-E05735.svg)](./docs/CHANGELOG.md)

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="./README.ru.md">Русский</a> ·
  <a href="./docs/WIKI.ru.md">Wiki</a> ·
  <a href="./docs/TECHNICAL_ARCHITECTURE.md">Архитектура</a> ·
  <a href="./docs/CHANGELOG.md">Changelog</a>
</p>

VoiceSub — Windows desktop-приложение, которое превращает речь в субтитры в реальном времени с опциональным переводом. Распознавание — через **Chrome/Edge Web Speech** или опциональный офлайн **Local ASR** (Parakeet / ONNX). Всё работает локально: bind по умолчанию `127.0.0.1:8765`, без cloud backend и аккаунтов.

Преемник SST Desktop `0.4.4`. Первый релиз VoiceSub: **`0.5.0`**. Текущая линия: **`0.6.0`**.

---

## Возможности

| Область | Что даёт |
| --- | --- |
| **Речь** | Chrome/Edge Web Speech worker или офлайн Local ASR (Parakeet / ONNX, CPU или CUDA) |
| **Перевод** | 13 провайдеров, до 5 линий перевода |
| **OBS** | Browser Source overlay + опциональные Closed Captions (WebSocket) |
| **Стиль** | Анимированные пресеты, стили по слотам, палитра темы |
| **TTS** | Native / Sonic playback; озвучка субтитров + Twitch chat TTS (до 5 каналов) |
| **Local ASR** | Wizard на `/local-asr`; режим `local_parakeet` на Эфире при `ready` |
| **Ops** | Экспорт diagnostics ZIP; локали UI en / ru / ja / ko / zh |

Компактный макет под второй монитор / узкое окно.

---

## Системные требования

- Windows 10 или 11 (x64)
- **Microsoft Edge WebView2 Runtime** (на Windows 11 обычно уже есть; NSIS-установщик может запустить bootstrapper на Windows 10)
- **Google Chrome** или Edge — только для Web Speech worker (не нужен, если используется только Local ASR)
- Доступ к микрофону
- Интернет — опционально для облачных провайдеров перевода; также для первой загрузки модели / ORT Local ASR

Python, Node.js и CUDA **не входят** в core-установщик. CUDA — опциональная загрузка модуля Local ASR.

---

## Быстрый старт

1. Установите из `VoiceSub_0.6.0_x64-setup.exe` (или последней сборки в папке релиза).
2. Запустите **VoiceSub.exe** — dashboard откроется на `http://127.0.0.1:8765/`.
3. В OBS добавьте **Browser Source** → `http://127.0.0.1:8765/overlay`.
4. При необходимости настройте перевод и стиль субтитров, нажмите **Start**.
5. Выберите распознавание:
   - **Web Speech** — держите окно browser worker открытым и видимым (разрешение микрофона выдаётся там).
   - **Local ASR** — **Модули → Local ASR**, завершите setup до `ready`, выберите Local ASR на Эфире, затем Start.

Пошаговый гайд: [Wiki (RU)](./docs/WIKI.ru.md) · [Wiki (EN)](./docs/WIKI.en.md)

---

## Локальные URL

| URL | Назначение |
| --- | --- |
| `http://127.0.0.1:8765/` | Dashboard |
| `http://127.0.0.1:8765/overlay` | OBS Browser Source |
| `http://127.0.0.1:8765/google-asr?autostart=1` | Browser Speech worker |
| `http://127.0.0.1:8765/tts` | TTS-модуль |
| `http://127.0.0.1:8765/local-asr` | Модуль Local ASR |

Примеры query для overlay: `?preset=single` · `?compact=1` · `?profile=default`

---

## Пути данных

| Путь | Содержимое |
| --- | --- |
| `user-data/config.toml` | Основные настройки |
| `user-data/profiles/` | Именованные профили |
| `user-data/modules/tts/` | Настройки TTS |
| `user-data/modules/local-asr/` | Config Local ASR, модели, ORT / CUDA runtime |
| `user-data/translation-cache/` | Кэш перевода |
| `logs/` | `core.log`, `runtime-events.log`, `session-latest.jsonl` |
| `bin/fonts/` | Шрифты субтитров |

SST `config.json` можно импортировать при первом запуске или из настроек. Legacy `local` / experimental → `browser_google`; `local_parakeet` сохраняется. Подробности: [Архитектура §7](./docs/TECHNICAL_ARCHITECTURE.md).

---

## Troubleshooting

| Симптом | Что проверить |
| --- | --- |
| Нет субтитров | Нажат **Start**; открыт worker (Web Speech) **или** Local ASR ready + выбран mic |
| Есть исходник, нет перевода | Перевод включён; активна хотя бы одна линия; credentials провайдера |
| Пустой OBS | Browser Source на `/overlay`; видимость во вкладке «Субтитры»; после обновления — reload source |
| Текст не исчезает после TTL / Stop | Обновите сборку; перезагрузите Browser Source |
| Порт занят | Освободите `8765` или смените bind (dev-сборки) |
| Нет Local ASR на Эфире | Модули → Local ASR: завершите wizard до `ready` |

Полный гайд: [Wiki → Troubleshooting](./docs/WIKI.ru.md).

---

## Документация

- [Wiki (RU)](./docs/WIKI.ru.md) / [Wiki (EN)](./docs/WIKI.en.md) — пользовательский гайд
- [Technical Architecture (RU)](./docs/TECHNICAL_ARCHITECTURE.md) / [(EN)](./docs/TECHNICAL_ARCHITECTURE.en.md)
- [Changelog (RU)](./docs/CHANGELOG.md) / [(EN)](./docs/CHANGELOG.en.md) — [Keep a Changelog](https://keepachangelog.com/ru/1.1.0/)

---

## Contributing

PR приветствуются. Для крупных изменений — сначала issue.

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
<summary><strong>Разработчикам — стек и сборка</strong></summary>

### Стек

| Слой | Технологии |
| --- | --- |
| Core | Rust workspace (`crates/voicesub-*`) + Axum HTTP/WS |
| Shell | Tauri 2 → `VoiceSub.exe` (NSIS) |
| Dashboard | Svelte 5 + Vite → `bin/dashboard/` |
| Worker | Svelte 5 → `bin/worker/` |
| Overlay | Vanilla HTML/JS → `bin/overlay/` |
| TTS | Svelte + Rust service + embedded Python sidecar |
| Local ASR | Svelte + `voicesub-asr-local` + ONNX Runtime (lazy download) |

Node.js — **только на этапе сборки**, не в установщике.

### Сборка из исходников

```powershell
npm install
npm run build          # dashboard + worker + TTS + Local ASR
npm run i18n:export    # scripts/i18n-source → locale JSON
npm run i18n:bundle    # overlay locales bundle
cargo test --workspace
build-release-msi.bat  # → NSIS setup.exe в release_root
```

Tauri `beforeBuildCommand`: `npm run build`. В bundle: `bin/dashboard`, `overlay`, `worker`, `tts`, `local-asr`, `fonts`, `modules`.

### Ключевые crates

`voicesub-runtime` · `voicesub-subtitle` · `voicesub-translation` · `voicesub-browser` · `voicesub-ws` · `voicesub-tts` · `voicesub-asr-local` · `voicesub-partial-emit` · `voicesub-obs`

`src-tauri/` — тонкая IPC-оболочка, без domain logic.

Источник версии: `voicesub-types::PROJECT_VERSION` = **`0.6.0`**.

Полный справочник: [Technical Architecture](./docs/TECHNICAL_ARCHITECTURE.md).

</details>
