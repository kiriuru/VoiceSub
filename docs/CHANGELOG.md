# Журнал изменений SST Desktop

Единая история изменений desktop-версии.

Этот файл является каноническим changelog для релизов SST Desktop. Версионные release notes в `docs/DESKTOP_RELEASE_CHANGELOG_*.md` остаются как delta-документы по конкретным релизам, но основной историей изменений считается этот файл.

## Unreleased

После ветки `0.3.0`: внутренняя модульность и поведение старта рантайма, **без** смены local-first продукта и **без** изменения публичного источника версии (`PROJECT_VERSION`).

### UI: выбор модели OpenAI

- провайдер OpenAI отдаёт курируемый список популярных моделей через `GET /api/openai/recommended-models`;
- панель настроек провайдера перевода на дашборде может заполнить поле `model` из этого списка без вызова OpenAI из браузера.

### Модульность рантайма (промежуточный этап)

- состояние метрик рантайма перенесено в `RuntimeMetricsController` (`backend/core/runtime/runtime_metrics_controller.py`);
- состояние подключения/сессии/generation/signature браузерного worker-а принадлежит `BrowserWorkerStateController` (`backend/core/runtime/browser_worker_state_controller.py`);
- `RuntimeOrchestrator` делегирует эти низкорисковые мутации контроллерам, не меняя форму payload статуса рантайма и поведение WebSocket;
- добавлены модульные тесты на владение метриками и состоянием browser worker (`tests/test_runtime_metrics_controller.py`, `tests/test_browser_worker_state_controller.py`);
- идентичность сессии рантайма, метки времени и записи экспорта перенесены в `RuntimeSessionController` (`backend/core/runtime/runtime_session_controller.py`), `RuntimeOrchestrator` делегирует завершённые записи экспорта и подготовку payload экспорта;
- добавлены тесты сессии/экспорта (`tests/test_runtime_session_controller.py`) и обновлено регрессионное покрытие экспортёра;
- счётчик сегментов, активный сегмент и учёт partial coalescing перенесены в `SegmentStateController` (`backend/core/runtime/segment_state_controller.py`);
- `RuntimeOrchestrator` делегирует учёт сегментов/partial в `SegmentStateController`, намеренно не меняя поведение ASR/audio/VAD/transcript;
- добавлены тесты состояния сегментов и partial (`tests/test_segment_state_controller.py`).

### Стабилизация рантайма P1 (фасад и контроллеры)

- `TranslationDispatcher` стал перезапускаемым: `stop()` больше не «ломает» диспетчер для следующих сессий; `start()` сбрасывает внутреннее состояние остановки, тесты покрывают сценарий `stop() -> start()`;
- config и профили пишутся атомарно (Windows: временный файл в той же папке + `os.replace()`), снижая риск частичной записи при обрыве питания или падении;
- повреждённый `user-data/config.json` восстанавливается автоматически:
  - невалидный JSON переносится в резервную копию с меткой времени;
  - восстанавливаются значения по умолчанию, приложение может загрузиться;
  - миграции и нормализаторы выполняются и для восстановленного payload;
- `RuntimeOrchestrator` стал тоньше как фасад над явными контроллерами в `backend/core/runtime/` с упорядоченной координацией жизненного цикла:
  - coalescing broadcast статуса рантайма (`RuntimeStateController`);
  - разрешение и фиксация режима ASR (`AsrModeController`);
  - жизненный цикл перевода и пересоздание диспетчера (`TranslationRuntimeController`);
  - обёртка презентации субтитров (`SubtitlePresentationController`);
  - единый исходящий fanout для WS/OBS (`OutputFanoutController`);
  - оркестрация конвейера транскриптов (`TranscriptController`);
  - явная абстракция источника речи + фабрика (`SpeechSource*`);
  - детерминированный порядок start/stop (`RuntimeLifecycleCoordinator`);
  - вынесенные помощники reset/session/task/audio/worker/export (см. техдок).
- `ConfigStateService` использует явную блокировку: активный in-memory снимок конфига безопасен при конкурентных операциях рантайма и настроек;
- очередь перевода: ограничение параллелизма по провайдеру и базовый rate limiting (защита от «пачек» при сохранении параллелизма по целевым языкам);
- проверки готовности локальных endpoint-ов кэшируются с фоновым обновлением, чтобы не блокировать горячие пути повторными пробами;
- `SubtitleRouter` разделён на:
  - `SubtitleLifecycleCore` (конечный автомат жизненного цикла, TTL/релевантность, promotion/expiry),
  - `SubtitlePresentation` (сборка payload, порядок, слоты стилей, слияние partial и финала),
  - `SubtitleRouter` (фасад публикации в overlay/WS, связывает core+presentation).

### Архитектура: последующие шаги

- монолитный `backend/config.py` заменён пакетом `backend/config/` с явными `defaults.py`, `secrets.py` и доменными нормализаторами в `backend/config/normalizers/`;
- `RuntimeOrchestrator` физически находится в `backend/core/runtime_orchestrator.py`, а `backend/core/subtitle_router.py` сохраняет логику жизненного цикла субтитров и shim только для совместимости импорта;
- внутренности жизненного цикла субтитров вынесены из `backend/core/subtitle_router.py` в `backend/core/subtitle_lifecycle_core.py` и `backend/core/subtitle_presentation.py`, `subtitle_router.py` остаётся фасадом и shim;
- оркестрация рантайма дальше разнесена по помощникам `backend/core/runtime/`, bootstrap подключает `backend/core/runtime_orchestrator.py` напрямую;
- провайдеры перевода вынесены в `backend/translation/providers/`, `backend/core/translation_engine.py` остаётся точкой совместимости/shim;
- документация описывает реальные профили лаунчера: «Быстрый старт (Browser Speech)», `NVIDIA GPU (CUDA)`, `CPU-only`, `Remote Controller`, `Remote Worker`.

### Дашборд и UX: последующие шаги

- вкладка Translation разделена на панель маршрутизации/слотов и отдельную панель настроек провайдера;
- каждый слот `translation_1 .. translation_5` — стабильная карточка с полями `enabled`, `target_lang`, `provider`, `label`;
- выбор слота перевода перенастраивает общий редактор настроек провайдера на провайдер этого слота; редактор можно переключать вручную, если слот не выбран;
- дашборд предупреждает, если включённые слоты используют провайдеры с незаполненными обязательными настройками;
- вкладка Style: тема интерфейса (светлая/тёмная) и палитра акцентного градиента для дашборда и окон Browser Speech;
- расширено покрытие i18n: прогресс рантайма, редактор слотов стиля, remote LAN, диагностика и прочий ранее захардкоженный текст;
- карточки слотов перевода показываются только для строк, явно добавленных в `translation.lines`;
- карточка прогресса рантайма в режимах Browser Speech переключается на компактный вид;
- смена языка UI сохраняется сразу, без обязательного глобального Save;
- для разработки: маршруты и статика отдаются с заголовками no-store, обычный refresh подхватывает правки;
- добавлена вкладка «Справка / Помощь» после «Tools & Data»: одна видимая тема за раз (wiki-панели): обзор, распознавание/тюнинг, перевод, субтитры/стиль, OBS, инструменты/диагностика, desktop/remote;
- в справке по remote зафиксирован порядок: worker → controller → проверка health worker → pairing/обновление состояния → синхронизация настроек → подготовка запуска → старт/проверка рантайма worker → открытые bridge-окна → старт рантайма на дашборде controller;
- тюнинг и тексты UI разделяют «быстрые» ползунки ощущения распознавания и точные тайминги ASR (последние в «Tools & Data»);
- готовность экспериментального провайдера перевода в бейджах остаётся `experimental`, а не нормализуется в `degraded`.

### Стили субтитров: последующие шаги

- добавлены встроенные эффекты появления: `slide_up`, `zoom_in`, `blur_in`, `glow` (общие для превью дашборда и OBS overlay).

### Хранилище desktop и выравнивание релиза

- пользовательские логи бэкенда и desktop — в корневом `logs/` (устаревший `user-data/logs/` мигрируется при старте);
- устаревшие корневые `logs/` мигрируются вперёд при старте лаунчера/рантайма;
- локальные модели — в `user-data/models/`;
- документация релиза и сценарий публикации отражают bootstrap-цели и текущую структуру desktop.

### Проверка обновлений

- bootstrap-лаунчер проверяет GitHub Releases и показывает диалог только при доступной более новой версии (Продолжить / Скачать);
- бэкенд: `POST /api/updates/check` для явного опроса GitHub Releases, сохранение `updates.latest_known_version` и `updates.last_checked_utc`.

### Перевод: последующие шаги

- конфигурация перевода поддерживает выбор провайдера на строку через `translation.lines`;
- у каждой строки стабильный `slot_id` (например `translation_1`), он — основной идентификатор для порядка и рендера в overlay;
- дубли целевых языков допустимы, если слоты разные;
- ключи кэша перевода включают `provider_name`, исключая коллизии при двух провайдерах на один язык;
- legacy `translation.provider` и `translation.target_languages` сохранены для совместимости и при необходимости восстанавливаются из нормализованных слотов;
- legacy `subtitle_output.display_order` по кодам языков мигрируется в id слотов перевода.

### Контракт старта рантайма

- `POST /api/runtime/start` принимает опциональный снимок `config_payload` вместе с `device_id`;
- дашборд отправляет текущий нормализованный in-memory конфиг при нажатии «Старт», чтобы изменения только в рантайме применялись без обязательного «Save Settings»;
- снимок применяется только в памяти, фиксируется метаданными активного конфига и **не** пишется в `user-data/config.json`, пока пользователь явно не сохранит настройки;
- предзагрузка remote-сессии читает `remote.session_id` и `remote.pair_code` из этого снимка, чтобы pairing следовал несохранённым правкам UI.

### Тесты и верификация

- API-тесты: `/api/runtime/start` использует несохранённый снимок конфига без изменения payload на диске;
- покрытие статуса рантайма: `active_config_source`, `active_config_persisted`, `active_config_hash`;
- архитектурные тесты: наличие и чистый импорт `backend/config/`, `backend/core/runtime/`, `backend/asr/parakeet/`, `backend/translation/`;
- регрессия путей desktop: корневой `logs/` и миграция `user-data/logs/` в потоке лаунчера/рантайма;
- на ветке прогнано:
  - `python -m compileall backend desktop tests`
  - `.\.venv\Scripts\python.exe -m unittest discover -s tests`
- результат: `231 tests`, `OK`;
- вывод ручной non-remote smoke зафиксирован в `docs/MANUAL_SMOKE_RESULTS_NON_REMOTE.md` (пункты только с микрофоном/OBS/браузером остаются NOT TESTED без фактического прогона).

### Пакет стабилизации non-remote рантайма

- согласованность жизненного цикла `RuntimeOrchestrator`: единая реализация `stop()`, идемпотентность, покрыто тестами;
- добавлены узкие тесты для:
  - канонического порядка start/stop `RuntimeLifecycleCoordinator`;
  - жизненного цикла non-remote SpeechSource/контроллеров (`BrowserSpeechSource`, `LocalParakeetSpeechSource`, `AudioCaptureController`, `ProcessingTasksController`);
  - регрессий разделения `SubtitleRouter` (partial/final/релевантность перевода, сброс, legacy display_order).
- добавлен `docs/MANUAL_SMOKE_CHECKLIST_NON_REMOTE.md` для воспроизводимого smoke без remote.

## 0.3.0

Архитектурный релиз с переносом backend на явные слои services/schemas/bootstrap, модульным frontend без шага сборки, миграциями конфига и экспортом схемы, новым слоем устойчивости рантайма/browser ASR и документированным experimental-путём браузерного worker.

### Основные изменения

- backend разделён на `api/routes`, `services`, `core`, `schemas` без смены базового local-first продукта;
- `app.state` больше не собирается вручную в одном `app.py`, а поднимается через централизованный bootstrap;
- config получил явные migrations `config_version` и экспорт JSON Schema;
- dashboard переведён с монолитного `app.js` на ES modules с `core/`, `dashboard/`, `panels/`, `normalizers/`;
- жизненный цикл Browser Speech вынесен в отдельный supervisor/session manager и стал устойчивее к `onend`, `no-speech`, reconnect и устаревшему состоянию worker;
- `/ws/events` и `/ws/asr_worker` получили более безопасную обработку сценариев reconnect, мёртвого сокета и устаревшей generation браузерного worker;
- логирование client-event в режиме best-effort и больше не должно валить backend из-за ошибок записи live event log;
- путь overlay/рантайма лучше переживает шторм дубликатов/устаревших событий и поздние обновления перевода;
- отдельная experimental-страница `/google-asr-experimental` включена в релиз как поддерживаемый experimental-путь на базе `SpeechRecognition.start(audioTrack)`;
- локальный AI-путь и `browser_google` не удалены; Parakeet остаётся доступным;
- неподдерживаемые эксперименты backend ASR убраны с активной продуктовой поверхности; остаются только Parakeet и режимы browser worker.

### Архитектура backend

- добавлены и подключены `backend/services/runtime_service.py`, `settings_service.py`, `asr_service.py`, `translation_service.py`, `diagnostics_service.py`, `export_service.py`, `overlay_service.py`, `model_manager_service.py`;
- введён `backend/core/app_bootstrap.py` как единая точка инициализации путей рантайма, менеджеров, сервисов и связывания orchestrator;
- выделены общие утилиты:
  - `backend/core/paths.py`
  - `backend/core/logging_setup.py`
  - `backend/core/api_errors.py`
  - `backend/core/redaction.py`
- `backend/runtime_paths.py` оставлен как совместимый shim поверх нового слоя путей;
- маршруты стали тоньше и делегируют оркестрацию сервисам приложения;
- `backend/api/routes_profiles.py` переведён на более структурированный payload ошибок API.

### Конфигурация, миграции, схема

- config переведён на явные migrations через `backend/core/config_migrations.py`;
- профили и основной config проходят общий pipeline миграции/нормализации;
- добавлен экспорт схемы через `backend/core/config_schema_export.py`;
- схема публикуется в `backend/data/config.schema.json`;
- расширены Pydantic schema-модули в `backend/schemas/` для config/runtime/asr/translation/overlay/diagnostics;
- migration v3 переводит `official_eu_parakeet_realtime` на `official_eu_parakeet_low_latency`;
- устаревшие настройки исторического backend ASR при нормализации возвращаются к поддерживаемым дефолтам Parakeet.

### Модульность фронтенда

- точка входа dashboard — `frontend/js/main.js`;
- новый стек модулей:
  - `frontend/js/core/`
  - `frontend/js/dashboard/`
  - `frontend/js/panels/`
  - `frontend/js/normalizers/`
- store/API/WebSocket/events/logging вынесены в отдельные модули;
- логика панелей разделена по доменам вместо разрастания одного файла;
- normalizers — отдельные чистые функции, удобные для тестов;
- стек без изменений по принципу:
  - plain HTML/CSS/JS
  - раздача через FastAPI static
  - без Node.js, React, Vite, Webpack и любого конвейера сборки.

### Устойчивость Browser Speech

- жизненный цикл распознавания в браузере вынесен в `frontend/js/browser-asr-session-manager.js`;
- введён supervisor с состояниями:
  - `idle`
  - `starting`
  - `running`
  - `stopping`
  - `restarting`
  - `backoff`
  - `fatal`
- убран старый хаотичный цикл `start/stop/onend`;
- `recognition.start()` больше не вызывается поверх `stopping`, а откладывается до контролируемого перезапуска;
- добавлены cooldown с учётом причины:
  - `normal_onend`
  - `settings_change`
  - `websocket_reconnect`
  - `watchdog_stall`
  - `no_speech`
  - `network`
- добавлена диагностика worker:
  - `generation_id`
  - `session_id`
  - `recognition_state`
  - `browser_supervisor_state`
  - `desired_running`
  - `pending_start`
  - `restart_count`
  - `no_speech_count`
  - `network_error_count`
  - `duplicate_partial_suppressed`
  - `duplicate_final_suppressed`
  - `late_forced_final_suppressed`
  - поля здоровья микрофона (`mic_track_ready_state`, `mic_track_muted`, `mic_rms`, `mic_active_recent_ms`, `last_mic_activity_at`)
- переподключения browser worker не должны оставлять рантайм в устаревшем `listening/stopping`;
- classic `/google-asr` в приоритете использует собственные настройки `localStorage`, затем зеркалит их в config бэкенда;
- experimental `/google-asr-experimental` синхронизирован с тем же базовым FSM и не должен ломаться из-за устаревшего subclass API.

### Устойчивость WebSocket и событий рантайма

- `backend/ws_manager.py` стал безопаснее при конкуренции и терпимее к disconnect/ошибкам send;
- мёртвые сокеты удаляются после `WebSocketDisconnect`, `RuntimeError`, `OSError`, `ConnectionResetError`, `BrokenPipeError`;
- повторный disconnect/close не должен валить менеджер;
- события рантайма/browser worker обрабатываются с учётом sequence и устаревания;
- лавина дубликатов `runtime_status -> listening` подавляется логикой coalescing;
- reconnect `/ws/events` не должен плодить активные client loops и старые таймеры;
- ошибки закрытия Windows уровня `WinError 10022` обрабатываются как очистка disconnect, а не как фатальный сбой рантайма.

### Логирование и диагностика

- `/api/logs/client-event` переведён в режим best-effort;
- проблемы записи live event log больше не должны давать backend `500`;
- `SessionLogger` создаёт каталог логов заранее, не держит проблемный file handle постоянно и считает отброшенные события;
- счётчики клиентских логов добавлены в диагностику рантайма;
- редактирование чувствительных полей (`token`, `secret`, `password`, `pair_code`, `api_key`, ключи вида credential);
- структурированные логи рантайма усилены для browser recognition, метрик рантайма и провайдер-специфичных путей.

### Согласованность overlay и перевода

- путь overlay/рантайма лучше защищён от несоответствия устаревшего перевода;
- поздние/устаревшие обновления перевода не должны так легко прилипать к новому сегменту источника;
- шум дубликатов рантайма не должен лишний раз дёргать payload overlay;
- subtitle router и overlay broadcaster получили дополнительное подавление/coalescing.

### Очистка поверхности ASR

- текущая поверхность ASR ограничена локальным Parakeet и двумя режимами browser worker;
- удалённые/неподдерживаемые эксперименты backend ASR вычищаются при миграции и save/load конфига;
- дашборд и схема больше не показывают устаревшие настройки транспорта backend ASR.

### Remote-режим и запуск

- remote mode сохранён как явное исключение только для LAN;
- синхронизация remote worker дополнительно фиксирует локальный AI-провайдер, чтобы worker не уходил в browser worker;
- запуск по умолчанию остаётся local-first;
- `start.bat` по смыслу не превращён в remote bootstrap;
- дашборд/overlay/browser worker по-прежнему обслуживаются локальным FastAPI backend.

### Документация

- обновлены `README.md` и `README.ru.md` под релиз `0.3.0`;
- обновлена полная техническая документация `docs/TECHNICAL_ARCHITECTURE.md`;
- обновлён delta changelog `docs/DESKTOP_RELEASE_CHANGELOG_0.3.0.md`;
- обновлена документация и тесты под актуальную структуру проекта.

### Тесты и верификация

Добавлено/обновлено покрытие для:

- архитектуры backend
- миграций конфига
- экспорта схемы конфига
- контракта browser worker
- browser ASR service и gateway
- coalescing событий рантайма
- очистки мёртвых сокетов WebSocket manager
- устойчивости session logger к сбоям
- модульной архитектуры frontend
- контракта логирования дашборда
- контракта статуса рантайма
- выбора провайдера ASR и очистки legacy-конфига
- remote-потока и версионирования

Проверка на актуальном наборе изменений:

- `python -m compileall backend tests`
- `.\.venv\Scripts\python.exe -m unittest discover -s tests -p "test_*.py"`

Результат:

- `135 tests`
- `OK`

## 0.2.9.x

История `0.2.9.*` остаётся в архивных релизных заметках и не ведётся в этом основном changelog-файле.
