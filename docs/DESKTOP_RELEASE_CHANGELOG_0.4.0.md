# SST Desktop 0.4.0

## Stream Subtitle Translator 0.4.0

EN:

This release updates the desktop bootstrap payload to app version **0.4.0** and adds a second Web Speech-only installer.

Included in this release:

- added **compact dashboard layout** (`ui.layout = compact`): vertical shell, icon navigation rail, sticky save/runtime chrome; layout choice is saved in config;
- desktop shell **resizes the main window** when switching standard ↔ compact (separate min/size targets for each layout);
- fixed **Web Speech quick start** so Local Parakeet cannot be selected afterward (`asr.desktop_profile_lock` is written to config and survives save/load; Recognition removes the local option until a GPU/CPU launch clears the lock);
- fixed **slow dashboard startup** in the desktop shell (panels mount immediately; settings and launch context load in the background instead of blocking on pywebview);
- backend: Browser ASR observability (trace/replay, stale ingress rejection), bounded WebSocket queues, preview-translation supersession;
- fix: browser speech worker WebSocket no longer drops immediately after connect.

### Desktop release format

- `Stream Subtitle Translator.exe` — bootstrap launcher (same startup profiles as before; payload inside is 0.4.0);
- `Stream Subtitle Translator Only Web.exe` — **new** bootstrap: starts Web Speech without the profile splash; compact splash only.

### Change history

- full changelog: [docs/CHANGELOG.md](./CHANGELOG.md)
- version notes: this file

---

## RU

Релиз обновляет payload внутри desktop bootstrap до версии приложения **0.4.0** и добавляет второй установщик только под Web Speech.

Что вошло:

- **компактный режим дашборда** (`ui.layout = compact`): вертикальная оболочка, боковая навигация иконками, липкий блок Start/Save; выбор режима сохраняется в config;
- **ресайз окна** desktop-shell при переключении standard ↔ compact (отдельные размеры и min-size для каждого режима);
- исправлен **Web Speech quick start**: после него нельзя выбрать Local Parakeet (`asr.desktop_profile_lock` пишется в config и переживает save/load; пункт local убирается из Recognition до запуска с GPU/CPU);
- исправлен **долгий старт дашборда** в desktop (панели сразу; настройки и launch context в фоне, без блокировки на pywebview);
- backend: наблюдаемость Browser ASR (trace/replay, отсев stale на ingress), bounded WebSocket, supersession preview-переводов;
- fix: WebSocket browser worker не обрывается сразу после connect.

### Формат desktop release

- `Stream Subtitle Translator.exe` — bootstrap (профили запуска те же; внутри — приложение 0.4.0);
- `Stream Subtitle Translator Only Web.exe` — **новый** bootstrap: сразу Web Speech, без splash выбора профиля.

### История изменений

- полный changelog: [docs/CHANGELOG.md](./CHANGELOG.md)
- заметки версии: этот файл
