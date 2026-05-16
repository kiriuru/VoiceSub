# SST Desktop 0.4.0

Delta для установочных **bootstrap-exe** поверх **0.3.2**. Исходники на GitHub — под `start.bat`; папка `desktop/` и скрипты сборки остаются локальными.

## Русский

### Вложения релиза

| Файл | Что нового в этой сборке |
|------|---------------------------|
| `Stream Subtitle Translator.exe` | Стандартный bootstrap **0.4.0** (те же профили запуска, что и раньше) + исправления ниже |
| `Stream Subtitle Translator Only Web.exe` | **Новый** one-file bootstrap: сразу Web Speech, без splash выбора профиля |

### Изменения 0.4.0 (не из 0.3.2)

**Backend (общий для desktop и `start.bat`)**

- Наблюдаемость Browser ASR: trace id, monotonic time, operational FSM, JSONL replay, отсев stale/overlap на ingress.
- Bounded WebSocket-очереди; preview-переводы с supersession.
- Исправление: `browser_asr_worker_connected()` — worker WebSocket не обрывается сразу после connect.

**Desktop-сборка (только в exe)**

- **Блокировка Parakeet** после Web Speech quick start / Only Web: `asr.desktop_profile_lock` в схеме config и после save/load; в Recognition убирается пункт Local Parakeet до следующего старта с GPU/CPU.
- **Быстрый дашборд**: панели сразу, `DesktopBridge` и настройки в фоне (без ожидания pywebview до 12 с).
- **Only Web.exe** — отдельная сборка и publish-скрипт (локально: `build-bootstrap-launcher-web-only.bat`, `publish-desktop-releases-web-only.ps1`).

`config_version` — **7**. Версия приложения — **0.4.0** (`backend/versioning.py`).

## English

### Release assets

| File | What is new in this build |
|------|---------------------------|
| `Stream Subtitle Translator.exe` | Standard **0.4.0** bootstrap (same startup profiles as before) plus the fixes below |
| `Stream Subtitle Translator Only Web.exe` | **New** one-file bootstrap: Web Speech only, no profile splash |

### 0.4.0 changes (not in 0.3.2)

**Backend (desktop and `start.bat`)**

- Browser ASR observability: trace ids, monotonic clocks, operational FSM, JSONL replay, stale/overlap ingress rejection.
- Bounded WebSocket queues; preview translation supersession.
- Fix: restored `browser_asr_worker_connected()` so the worker socket stays up after connect.

**Desktop installers only**

- **Parakeet lock** after Web Speech quick start / Only Web: `desktop_profile_lock` persists in config; Local Parakeet hidden in Recognition until a GPU/CPU launch clears the lock.
- **Faster dashboard**: panels mount immediately; bridge and settings load in the background.
- **Only Web.exe**: separate build/publish scripts (local-only in the dev tree).

`config_version` stays **7**. App version **0.4.0**.
