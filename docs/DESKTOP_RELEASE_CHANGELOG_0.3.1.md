# SST Desktop 0.3.1

## Stream Subtitle Translator 0.3.1

EN:

`0.3.1` is a stabilization release on top of `0.3.0`. Architectural milestones from `0.3.0` (runtime controllers, `SubtitleRouter` split, `backend/translation/`, update API, Help tab, Web Speech supervisor, isolated Chrome worker) are **not repeated here**. Public `/api` and WebSocket contracts are unchanged.

Included in this release:

- `PROJECT_VERSION = "0.3.1"`.
- Bootstrap update check ignores legacy `v2.x` tags when the built-in launcher is on the `0.x` line (no false “newer than 0.3.x” from old `v2.8.x` releases).
- **Web Speech / Chrome worker hardening:** `HIGH_PRIORITY_CLASS`; Windows EcoQoS opt-out; disabled Chrome occlusion/efficiency feature gates; in-worker `wakeLock`, network preflight after repeated `network` errors, `voice_below_recognition_threshold` health signal; default `max_browser_session_age_ms` lowered to `180000`.
- **Translation cache:** `cache_manager.py` — in-memory LRU with debounced disk persistence.
- **Logging:** `structured_log_compact.py` for runtime log compaction.
- **TranslationDispatcher:** restart-safe `stop()`; per-provider concurrency limits and basic rate limiting.
- **Subtitle effects:** `slide_up`, `zoom_in`, `blur_in`, `glow`.
- **Frontend polish:** translation panel / slot cards, i18n, small ASR/runtime/style fixes.
- **Docs:** unified `CHANGELOG.md` and `TECHNICAL_ARCHITECTURE.md`.

### Desktop release format

- single `Stream Subtitle Translator.exe` bootstrap launcher;
- on first launch the launcher extracts the managed runtime next to the executable.

### Change history

- full changelog: [docs/CHANGELOG.md](./CHANGELOG.md)
- version notes: this file

### Already in 0.3.0 (not new in 0.3.1)

Runtime controller decomposition, translation provider package, `ConfigStateService`, `/api/updates/check`, OpenAI model helper routes, `translation_1..translation_5` slots, UI theme/palette, Help tab, Web Speech supervisor, diagnostics export — see `0.3.0` release notes in `docs/CHANGELOG.md`.

---

## RU

`0.3.1` — релиз стабилизации поверх `0.3.0`. Архитектурные изменения `0.3.0` (контроллеры runtime, split `SubtitleRouter`, `backend/translation/`, update API, Help, supervisor Web Speech, изолированный Chrome) **здесь не дублируются**. Публичные `/api` и WebSocket-контракты не меняются.

Что вошло:

- `PROJECT_VERSION = "0.3.1"`.
- Bootstrap при проверке обновлений игнорирует legacy-теги `v2.x` на линии `0.x`.
- **Web Speech / Chrome worker:** `HIGH_PRIORITY_CLASS`; opt-out EcoQoS; отключённые feature gates Chrome; `wakeLock`, network preflight, health `voice_below_recognition_threshold`; `max_browser_session_age_ms` по умолчанию `180000`.
- **Кеш перевода:** `cache_manager.py` — LRU в памяти, debounce записи на диск.
- **Логи:** `structured_log_compact.py`.
- **TranslationDispatcher:** restart-safe `stop()`; лимиты параллелизма по провайдеру.
- **Эффекты субтитров:** `slide_up`, `zoom_in`, `blur_in`, `glow`.
- **Frontend:** мелкие правки панелей, i18n.
- **Документация:** `CHANGELOG.md`, `TECHNICAL_ARCHITECTURE.md`.

### Формат desktop release

- один bootstrap `Stream Subtitle Translator.exe`;
- при первом запуске launcher раскладывает managed runtime рядом.

### История изменений

- полный changelog: [docs/CHANGELOG.md](./CHANGELOG.md)
- заметки версии: этот файл

### Уже было в 0.3.0 (не новое в 0.3.1)

Декомпозиция runtime, пакет переводчиков, `ConfigStateService`, update API, слоты перевода, тема UI, Help, supervisor Web Speech, diagnostics export — см. `docs/CHANGELOG.md` (секция `0.3.0`).

### Проверка (на момент релиза)

- `python -m unittest discover -s tests` — **283** tests, `OK`
