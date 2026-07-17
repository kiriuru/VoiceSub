# Журнал изменений

Все заметные изменения этого проекта документируются в этом файле.

Формат основан на [Keep a Changelog](https://keepachangelog.com/ru/1.1.0/),
проект следует [Semantic Versioning](https://semver.org/lang/ru/).

<p align="center">
  <a href="./CHANGELOG.en.md">English</a> •
  <a href="./CHANGELOG.md">Русский</a>
</p>

Файл охватывает desktop-линию: **VoiceSub** (с `0.5.0`) и более ранние релизы **SST Desktop** (до `0.4.4`).

## [Unreleased]

## [0.6.0] - 2026-07-18

### Added

- Модуль **Local ASR** (`/local-asr`) — офлайн Parakeet TDT через ONNX Runtime (CPU / опционально CUDA), wizard установки, карточка Modules с badge ready / CPU / CUDA.
- Режим Live `local_parakeet` при `ready` модуля (in-process mic + VAD + decode; без Chrome Web Speech worker).
- Protected HTTP API `/api/asr/local/*` — status, config, deps, загрузка модели, EP probe, список mic, test bench.
- Настройки модуля в `user-data/modules/local-asr/`; `asr.mode` проекта — в `user-data/config.toml`.
- Crate `voicesub-partial-emit` (политика partial `word_growth`) в существующий путь subtitle / translation / overlay.
- Пресеты latency `low` / `balanced` / `quality`, hallucination filter, emit telemetry, checklist setup (deps → model → mic test → final).
- Installer `VoiceSub_0.6.0_x64-setup.exe`.

### Changed

- Документация и wiki: Local ASR отмечен как shipped; Technical Architecture §18 описывает модуль.
- Импорт SST JSON сохраняет `local_parakeet`; legacy `local` / experimental по-прежнему → `browser_google`.

## [0.5.5] - 2026-06-26

### Added

- Отдельный Tauri IPC pump с trailing-edge коалесингом dashboard `overlay_update` (по умолчанию 90 ms; `VOICESUB_OVERLAY_IPC_MIN_INTERVAL_MS`; `0` — выкл.). OBS `/ws/events` по-прежнему получает каждый кадр.
- Метрики bus lag и overlay IPC coalescing в `/api/runtime/status`.

### Changed

- Lifecycle субтитров выполняется до WS/IPC fanout; broadcast partial `transcript_update` — async, ingest не блокируется.
- Lock-free WS `global_sequence`; более безопасный debounced snapshot resync после bus lag.
- Dashboard пропускает частый HTTP poll runtime при активном Tauri IPC (остаётся safety-net 30 s).

### Fixed

- Ошибки Web Speech `audio-capture` — автоповтор с экспоненциальным backoff вместо полной остановки распознавания.

## [0.5.4] - 2026-06-21

### Added

- Per-window маршрутизация Tauri `runtime-event` (dashboard vs окно TTS).
- Leading-edge коалесинг partial `transcript_update` (по умолчанию 90 ms; `VOICESUB_TRANSCRIPT_PARTIAL_MIN_INTERVAL_MS`).
- Browser worker: reap осиротевшего PID при старте; flush длинных сегментов после монологов; hardening overlap ASR handoff.
- Расширенные настройки Web Speech в dashboard; диагностика ingest latency при полном логировании.

### Changed

- Live subtitle WebSocket fanout — только `overlay_update`; ASR — только `transcript_update`.
- `subtitle_payload_update` — только Tauri IPC snapshot/replay (не публикуется на `/ws/events`).
- Приоритет процесса browser worker — `ABOVE_NORMAL_PRIORITY_CLASS`.
- Диагностические timestamps — строки RFC 3339 вместо epoch seconds.
- Громкость TTS до 150%; фильтры Twitch-чата (mentions, digits, links) и hardening IRC reconnect.
- Очистка deprecated ключей timing lifecycle субтитров в config.

### Removed

- Дублирующие live broadcast: `transcript_segment_event` и `subtitle_payload_update` на `/ws/events`.
- JS queue pump TTS-модуля и deprecated TTS IPC.

### Fixed

- Корректность HTTP/WS fanout; лог Twitch-чата в модуле TTS.
- Надёжность TTS pipeline (prefetch, config I/O, порядок audio-chunks).

## [0.5.3] - 2026-06-17

### Added

- Завершение loopback API auth для protected `/api/*` (`x-voicesub-token`).
- Проверка обновлений через GitHub Releases (`POST /api/updates/check`) и баннер в dashboard.
- Навигация Material 3 на dashboard.
- Диагностика background-tasks на HTTP status.

### Changed

- UI Browser Speech worker; loopback/стили TTS-модуля; автопереподключение Twitch IRC.
- Toolchain edition **Rust 2024**; CI и commit-convention docs.
- Миграция: для protected HTTP helpers dashboard нужен loopback token (trusted pages инжектят `window.__VOICESUB_API_TOKEN__`).

### Fixed

- Hardening логирования OBS overlay (follow-up).
- Чистка мёртвого кода и неиспользуемых i18n-ключей.

## [0.5.2] - 2026-06-14

### Added

- Loopback API auth + проверки overlay liveness.
- Rust TTS speech pipeline на hot path; улучшения RuntimeEventBus snapshot.
- Стабильность запуска browser worker и telemetry overlap / browser-trace.

### Changed

- Алгоритм отправки OBS Closed Captions.

### Fixed

- TTS / Twitch поверх 0.5.1; polish UI dashboard.

## [0.5.1] - 2026-06-13

### Added

- Native dual-sink TTS (speech + Twitch) через Rust/cpal; режим **Sonic** (tempo без смены pitch).
- Twitch multi-channel (до 5 IRC JOIN на одно OAuth) с hot-apply фильтрами чата.
- Resource telemetry bar; политика power/memory WebView2; ротация логов по размеру.
- Списки top-20 языков для перевода / Web Speech; стабильные error codes OBS CC с i18n в UI.

### Changed

- TTS `playback_mode: "browser"` мигрирует в `sonic` при загрузке; путь HTMLAudio удалён.
- Twitch legacy `channel` → `channels[0]`; сохранение цифр в chat TTS.
- По умолчанию compact client logging (TTS UI traces — только при full logging).

### Fixed

- TTS enqueue IPC при пустом `dropped_ids`; детект языка Twitch на link-only строках.
- Утечки mic monitor и более чистый abort Web Speech при stop worker.

### Removed

- Путь TTS через browser `HTMLAudio` / `setSinkId`.

## [0.5.0] - 2026-06-10

Первый релиз VoiceSub (преемник SST Desktop `0.4.4`). Стек и поставка новые; смысл subtitle/translation сохранён.

### Added

- Desktop-приложение Rust + Tauri 2 (`VoiceSub.exe`, NSIS `VoiceSub_{version}_x64-setup.exe`).
- Svelte 5 dashboard, vanilla OBS overlay, Svelte Web Speech worker (`/google-asr`).
- TTS-модуль (`/tts`) с Twitch chat TTS; OBS Closed Captions (`voicesub-obs`).
- TOML config (`config_version` 8), импорт SST `config.json`, профили, diagnostics ZIP.
- Локали UI: en, ru, ja, ko, zh; проверка обновлений через GitHub Releases.

### Changed

- Продукт переименован в **VoiceSub**; bind по умолчанию `127.0.0.1:8765`.
- Production ASR в core: `browser_google` (Chrome/Edge worker).

### Removed

- FastAPI / pywebview / PyInstaller SST desktop stack из активного core.
- Legacy local ASR, remote controller/worker, experimental browser routes (архив в `legacy/`).
- Splash-профили запуска.

## [0.4.4] - 2026-05-31

> Замороженная линия SST Desktop. Активная разработка продолжается как VoiceSub.

### Security

- SSRF-политика для OpenAI helper model routes при LAN bind.

### Added

- Общий stale-guard overlay WebSocket; bridge desktop context в store.
- Локали ja / ko / zh для dashboard, worker и overlay.

### Changed

- Launcher desktop разделён на модули; reconnect overlay сохраняет последний кадр.

### Fixed

- Баннер ошибок bootstrap в dashboard; тесты безопасности bind/profile paths.

## [0.4.3] - 2026-05-27

### Added

- Desktop profile lock и hardening launcher (SST).

### Fixed

- Follow-up стабильности overlay и runtime на линии 0.4.2.

## [0.4.2] - 2026-05-25

### Added

- Дальнейший polish SST desktop к замороженной линии 0.4.x.

### Fixed

- Крайние случаи reconnect browser worker и overlay.

## [0.4.1] - 2026-05-20

### Added

- Инкрементальные возможности SST Desktop и миграции config поверх 0.4.0.

### Fixed

- Патчи надёжности dashboard и worker.

## [0.4.0] - 2026-05-16

### Added

- Feature set SST Desktop 0.4.0 (линия config_version к 7).

### Changed

- Шаги архитектуры и упаковки к baseline 0.4.4.

## [0.3.2] - 2026-05-14

### Fixed

- Патчи стабильности SST Desktop после 0.3.1.

## [0.3.1] - 2026-05-12

### Fixed

- Follow-up фиксы SST Desktop после модульного релиза 0.3.0.

## [0.3.0] - 2026-05-08

### Added

- Модульные FastAPI backend services и frontend module stack (SST).
- Supervisor FSM сессии Browser Speech; миграции config / export schema.

### Changed

- Более тонкие API routes; общие utilities paths, logging, redaction.

### Fixed

- Cleanup disconnect WebSocket; coalescing runtime events; best-effort client log.

### Removed

- Неподдерживаемые backend ASR experiments с активной поверхности продукта.

## [0.2.9.2] - 2026-04-30

Более ранняя история `0.2.9.*` SST Desktop остаётся в архивных GitHub release notes и здесь не развёрнута.

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
