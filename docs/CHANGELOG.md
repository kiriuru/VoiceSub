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

### Fixed

- Local ASR: `max_segment_ms` снова **5500** (как в UI / SST). Значение **120000** из пресетов/дефолта не давало force-final — партиклы росли минутами без Final при «липком» WebRTC VAD; при загрузке конфига ровно `120000` лечится обратно в `5500`.

## [0.6.0] - 2026-07-18

### Added

- Шрифты в `bin/fonts/`: **IBM Plex Mono**, **IBM Plex Serif**, **Source Code Pro** — для dual-script пресетов и кириллицы.
- Эффекты субтитров `pulse` и `reveal`.
- Тема UI: выбор шрифта интерфейса (`ui.font_family`) — применяется к dashboard, Web ASR, TTS и Local ASR через `ui_config_sync`.
- Диалог «О VoiceSub» (credits) из аватара в nav rail.
- Встроенная **Помощь**: quick-start чеклист и тематические карточки (распознавание, перевод, субтитры, стиль, OBS, инструменты) вместо сплошного prose.
- Модуль **Local ASR** (`/local-asr`) — офлайн Parakeet TDT через ONNX Runtime (CPU / опционально CUDA), wizard установки, карточка Modules с badge ready / CPU / CUDA.
- Режим Live `local_parakeet` при `ready` модуля (in-process mic + VAD + decode; без Chrome Web Speech worker).
- Protected HTTP API `/api/asr/local/*` — status, config, deps, загрузка модели, EP probe, список mic, test bench.
- Настройки модуля в `user-data/modules/local-asr/`; `asr.mode` проекта — в `user-data/config.toml`.
- Crate `voicesub-partial-emit` (политика partial `word_growth`) в существующий путь subtitle / translation / overlay.
- Пресеты latency `low` / `balanced` / `quality`, hallucination filter, emit telemetry, checklist setup (deps → model → mic test → final).
- Китайские провайдеры перевода с бесплатными квотами: `baidu_translate`, `youdao_translate`, `tencent_tmt` и `caiyun_translator` (zh/en/ja); всего **17** провайдеров, группа **China / Free-tier** в dashboard (i18n hints/status).
- Кнопка «Открыть сайт провайдера / API-ключи» у облачных провайдеров (Google, Azure, DeepL, OpenAI, OpenRouter, LibreTranslate, Baidu, Youdao, Tencent, Caiyun и др.; локальные LLM без ссылки).
- LLM: галочка **Override default subtitle prompt** (скрываемый custom prompt) для OpenAI / OpenRouter / LM Studio / Ollama.
- OpenAI: кнопка **Show models** — живой список через `POST /api/openai/models` (официальный `/v1/models`, фильтр chat-моделей); curated list обновлён по каталогу OpenAI 2026 (`gpt-5.6-*`, `gpt-5.4-*`, …); галочка **Show all chat models**.
- LM Studio: кнопка **Test connection** — проверка `base_url` и загрузка списка моделей.
- Installer `VoiceSub_0.6.0_x64-setup.exe`.

### Fixed

- Local ASR / runtime idle CPU: heartbeat больше не гоняет полный `env_check` (скан CUDA/DLL) на каждом тике — `diagnostics()` и `GET /api/asr/local/status` используют кэш статуса; поиск DLL индексирует каталог один раз; перечисление микрофонов в `spawn_blocking`; окно Local ASR откладывает mic list до первой отрисовки; dialog open/close без re-entrancy spin.
- Local ASR: утечка памяти/CPU WebView2 при открытии модуля — `setLocale` стал идемпотентным (иначе `sst:locale-changed` + BroadcastChannel зацикливались); модуль больше не подключается к `/ws/events` для UI sync (хватит Tauri IPC).
- TTS: то же — отключён `/ws/events` для UI sync (IPC уже шлёт `ui_config_sync`); locale sync идёт через `applyDashboardLocale` / идемпотентный `setLocale` (утечки как у Local ASR не было за счёт guard, но WS зря принимал overlay/runtime).
- Main dashboard: той же утечки нет (publisher UI sync, без subscribe/`sst:locale-changed` loop; `ui_config_sync` в store игнорируется); `applyUiFromConfig` пропускает повторное применение theme/locale/sync, если presentation signature не менялась.
- Старт/выход: `env_check` больше не блокирует создание Local ASR service (CUDA Toolkit bin scan); DLL lookup снова сначала проверяет прямой путь; dashboard применяет settings/theme до ожидания runtime status; last-known theme из localStorage до HTTP.
- Инструменты и данные: корректный сброс success/error; подтверждение load/overwrite профилей; запрет удаления `default`; валидация имён; предупреждение при импорте redacted config; disable import при busy; статус Local ASR; убран дубль stale-dropped; полное логирование применяется live после Save (без ложного «нужен перезапуск»); save/delete не показывают success, если не удалось обновить список профилей; `diagnostics_update` сохраняет `local_module` / `active_mode`; модальное окно success/error объясняет, куда сохранился файл (Downloads / `user-data/exports` / `user-data/profiles`).
- Профили: seed/upgrade sparse `default.json` из полных factory defaults; отклонение зарезервированных имён Windows.
- Экспорт diagnostics: уникальные имена `diagnostics-{secs}_{ms}.zip`; хранение последних 12 ZIP (prune best-effort, экспорт не падает из‑за очистки); понятные HTTP-ошибки delete/export.
- Docs: профили — `{name}.json` (не `.toml`); retention diagnostics ZIP задокументирован.
- Dashboard: смена чекбоксов/селектов больше не сбрасывает размер и позицию окна (resize+center только при смене `ui.layout`).
- Смена языка UI сохраняет текущий in-memory конфиг (не затирает импорт/профиль через `lastSavedConfig`).
- Web Speech advanced settings больше не форсят `asr.mode = browser_google`.
- OBS overlay: stale-guard активируется после загрузки module-скрипта; таймеры runtime-gone не копятся.
- Browser worker: autostart timer и lifecycle listeners очищаются в `destroy()`.
- Scroll-spy секций использует scroll-контейнер shell; вкладка Subtitles сразу открывает панель с верхними вкладками Субтитры/Стиль (без промежуточного hub).
- Иконки основных вкладок (nav rail / bottom nav) увеличены ~15%.
- Скролл TTS / Local ASR / Web Speech worker: `overflow: hidden` только у dashboard standard shell (не глобально на `html/body`).
- Local ASR: кнопки «Закрыть» (setup) и «Проверить снова» в стиле остального UI.
- Диалоги на тёмной теме: читаемый контраст (runtime «Подробнее», credits, Local ASR status/alert/setup) — `color-scheme` + явный цвет текста вместо UA `CanvasText`.
- Тема UI в Local ASR и TTS применяется сразу без «Сохранить» (IPC + WS fallback); i18n для `style.ui_theme.font` / `font.default`.
- Browser Google ASR lifecycle:
  - ошибка launch Chrome сбрасывает `runtime_running` и останавливает browser speech ingest (как у Local ASR);
  - PID tracking / `browser-worker.pid` очищаются только если Chrome реально мёртв; живой процесс после failed `taskkill` сохраняется для orphan reap;
  - IPC `launch_browser_worker` пишет PID в общий оркестратор и убивает предыдущий worker (без второго orphan Chrome);
  - старт Local ASR сначала reap leftover Chrome;
  - `generationId` бампается на каждом controlled start; отмена pending restart — через `stopEpoch` (user/control stop);
  - при замене WS-транспорта outbound предыдущего соединения дропается **без** `stop` (не убивает recognition на reconnect).
- **Замена слов (до перевода):** кэш Aho-Corasick/regex; CJK при дефолтном whole-words; корни; маска вида `fuck`→`f*ck` (не `***`); `f*ck` не разворачивается обратно. Субтитры/lifecycle без изменений.
- IPC ACL: `get_loopback_api_token` снова allowlisted (fallback без HTML injection).
- `runtime-event`: `listen` → buffer → snapshot → drain (live не затирается stale snapshot); dashboard snapshot предпочитает `overlay_update`; TTS snapshot только `runtime_update` + `twitch_connection_update`.
- `tts-speech-activity` / `playback-finished` — только `emit_to(tts)` (не global emit в main/local-asr).
- Twitch chat → `RuntimeEventBus` only (не флудит OBS `/ws/events`); connection updates по-прежнему в hub для replay.
- Lag-resync: pending-очередь (последний sync не дропается); coalesced overlay discard на `Lagged`, чтобы таймер не откатил UI после snapshot.
- Имя `Jet Brains Mono` выровнено с каталогом шрифтов; алиас `JetBrains Mono` при нормализации.
- OBS overlay сохраняет `style_slot` / `slot_id`; dashboard preview вызывает `disposeRenderContainer` при `render().empty`.
- Рендерер: `colorToRgba` (named/`rgb()`/`#rrggbbaa`); emoji по code-point; whitespace-only отфильтрованы; `inferStyleSlot` + `slot_id`; fast path не трогает disconnected surface.
- LM Studio / Ollama: JIT-загрузка модели больше не обрывается дефолтными 10s (`Engine protocol startup was aborted` / `Model is unloaded`); для local providers floor таймаута **≥120s**, в запрос LM Studio добавлен `ttl`.
- Кнопки setup-ссылок открывают системный браузер (allowlist `open_external_https_url` расширен хостами консолей провайдеров).
- Baidu Translate: POST form-urlencoded вместо GET; маппинг `sv` → `swe`; Youdao корректно читает числовой `errorCode`.
- Persistent-кэш перевода больше не затирается при каждом старте runtime; при неизменённых настройках переживает рестарт.
- Диспетчер перевода не утекает по `active_jobs` при повторном submit того же sequence; overflow очереди не держит dispatcher lock через relevance-проверки.
- Live apply настроек перевода ждёт engine lock (API keys / lines не пропускаются молча) и обновляет лимиты concurrency провайдеров.
- HTTP-таймауты перевода уважают `timeout_ms` (per-request timeout проведён через все провайдеры).
- Readiness-проба local LLM принимает hostname вроде `localhost` (DNS), а не только IP.
- Preview supersession устойчив к edge-case со счётчиками generation; short-circuit empty/identical-lang больше не помечается ложным cache hit.

### Changed

- Панель стилей: компактная сетка числовых полей; выравнивание и эффект в одной строке.
- Каталог built-in стилей пересобран (**20** пресетов): тематические dual-script стеки (Film Noir, Retro Terminal, Fallout, Anime Stream и др.); однотипные тёмные плашки сведены к **4 материалам** — Max Contrast, Podcast Subtle (пергамент), Glass Frost (молочный лёд ~44%), Twitch Lower-Third (`#9146FF` + Oswald). Удалены `sakura_soft`, `minimal_mono`, `editorial_news` (миграция → `meeting_soft` / `glass_frost` / `dark_cinema`).
- **Retro Terminal**: кириллица через **IBM Plex Serif Regular**.
- Latin-only лица в `/project-fonts.css` объявляют `unicode-range`, чтобы кириллица шла в следующий face стека (Plex / Ubuntu / Noto / Comfortaa…).
- Толщина обводки: шкала **ASS/Aegisub 0–4 px** (шаг 0.1).
- Эффекты: `fade` только opacity; `glow` из цвета заливки; на OBS partials тяжёлые `blur_in`/`glow` упрощаются до `fade`; учёт `prefers-reduced-motion`.
- Tauri IPC capabilities разделены по окнам: **main** (полный shell), **tts** (playback/Twitch/snapshot), **local-asr** (token + allowlisted URLs).
- TTS: HTTP poll `/api/runtime/status` только при мёртвом `runtime-event`; speech-context poll реже при живом IPC (focus по-прежнему обновляет сразу).
- Browser Speech worker — **только Google Chrome** (`/google-asr`): удалён маршрут `/google-asr-edge`; import `browser_google_edge` → `browser_google`; orphan reap только для `chrome.exe`.
- CPU affinity browser worker — **opt-in** (`VOICESUB_BROWSER_AFFINITY` / `VOICESUB_BROWSER_AFFINITY_MASK`), по умолчанию выкл.
- `runtime-event` routing: окно **local-asr** получает `ui_config_sync` (живая тема/локаль/шрифт); `/ws/events` реплеит последний `ui_config_sync` при подключении.
- Тексты Помощи обновлены под Local ASR / Modules / замену слов; i18n HelpPanel реактивен к смене локали.
- Документация и wiki: Local ASR отмечен как shipped; Technical Architecture §18 описывает модуль.
- Импорт SST JSON сохраняет `local_parakeet`; legacy `local` / experimental по-прежнему → `browser_google`.
- Потолок `timeout_ms` / HTTP client для перевода: **300s** (было 60s); UI Settings и config normalize синхронизированы.
- Путь persistent-кэша перевода — `user-data/translation-cache/` (legacy `user-data/cache/translation_cache.json` копируется один раз при апгрейде).
- Ключи кэша хешируют исходный текст; flush кэша при drop engine; `used_default_prompt` / `override_prompt` для LLM-провайдеров.
- DeepL мапит UI-коды языков (`en`/`zh-cn`/`pt`, …) в API targets и выбирает Free vs Pro URL по ключу (`:fx` → free).
- Google Cloud Translation v3 раскрывает короткие model id в full resource names; добавлены i18n-лейблы настроек Google v3.
- Azure / LibreTranslate мапят китайские UI-коды (`zh-Hans`/`zh-Hant`, `zh`/`zt`); readiness показывает soft-warnings для пустого Azure region и публичного LibreTranslate.

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
