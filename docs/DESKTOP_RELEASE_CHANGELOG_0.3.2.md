# SST Desktop 0.3.2

## Stream Subtitle Translator 0.3.2

EN:

`0.3.2` is a feature release on top of `0.3.1`. Public HTTP routes and the local-first baseline are unchanged; persisted `config.json` gains `source_text_replacement` (`config_version = 7`).

Included in this release:

- `PROJECT_VERSION = "0.3.2"`.
- **Config `config_version = 7`:** `source_text_replacement` (`enabled`, `include_builtin`, `case_insensitive`, `whole_words`, `pairs`).
- **Post-ASR word replacement:** applied in `TranscriptController` before subtitles, translation, and OBS captions; bundled `backend/data/source_text_builtin_pairs.json`; dashboard UI under **Tools & Data**.
- **Web Speech worker:** `browser-web-speech-recognition-policy.js`; `browser-asr-session-manager.js` buddy slot and Chrome error handling updates.
- **Subtitle presets:** `accessibility_high_contrast`, `dark_cinema`, `meeting_soft` in `subtitle_style.py`.
- **Docs:** `docs/TECHNICAL_ARCHITECTURE.md` (Parakeet/VAD/segment queue), README files, `docs/CHANGELOG.md`.

### Desktop release format

- single `Stream Subtitle Translator.exe` bootstrap launcher;
- on first launch the launcher extracts the managed runtime next to the executable;
- Browser Speech opens as a separate Chrome window with a visible address bar (unchanged invariant).

### Change history

- full changelog: [docs/CHANGELOG.md](./CHANGELOG.md)
- version notes: this file

---

## RU

`0.3.2` — функциональный релиз поверх `0.3.1`. Публичные `/api` и local-first baseline сохранены; в `config.json` появляется `source_text_replacement` (`config_version = 7`).

Что вошло:

- `PROJECT_VERSION = "0.3.2"`.
- **Конфиг `config_version = 7`:** секция `source_text_replacement`.
- **Пост-ASR замена слов:** `TranscriptController` до субтитров/перевода/OBS; `backend/data/source_text_builtin_pairs.json`; UI на вкладке «Инструменты и данные».
- **Web Speech worker:** `browser-web-speech-recognition-policy.js`; доработки `browser-asr-session-manager.js` (buddy-слот, ошибки Chrome).
- **Пресеты субтитров:** `accessibility_high_contrast`, `dark_cinema`, `meeting_soft`.
- **Документация:** `docs/TECHNICAL_ARCHITECTURE.md`, README, `docs/CHANGELOG.md`.

### Формат desktop release

- один bootstrap `Stream Subtitle Translator.exe`;
- при первом запуске launcher раскладывает managed runtime рядом;
- Browser Speech — отдельное окно Chrome с адресной строкой (инвариант без изменений).

### История изменений

- полный changelog: [docs/CHANGELOG.md](./CHANGELOG.md)
- заметки версии: этот файл

### Проверка (на момент релиза)

- `python -m unittest discover -s tests` — **298** tests, `OK`
