# План реализации Remote LAN

Этот план ограничен LAN-only remote режимом и сохраняет local mode стабильным по умолчанию.

## Шаг 1 — Основа и изоляция (baseline completed)

- Add `remote` config section with strict normalization.
- Keep default startup on `127.0.0.1`.
- Add explicit runtime role selection (`disabled`, `controller`, `worker`).
- Add dedicated startup wrappers for controller/worker.
- Add remote diagnostics endpoint and include diagnostics in health/runtime status responses.

## Шаг 2 — Сигналинг и сессия (baseline completed)

- Add pairing session creation (`session_id` + `pair_code`).
- Add worker pairing verification endpoint.
- Add controller/worker heartbeat endpoint.
- Track session state (`controller_online`, `worker_online`, expiry).

## Шаг 3 — Транспорт аудио (baseline in progress)

- Controller captures selected microphone stream for remote session.
- Controller sends microphone audio to worker using WebRTC.
- Worker receives WebRTC audio stream as ASR input source.
- Keep reconnect and timeout behavior explicit for unstable LAN.

Implemented baseline:
- signaling websocket relay (`/ws/remote/signaling`)
- worker audio ingest websocket (`/ws/remote/audio_ingest`)
- controller bridge page (`/remote/controller-bridge`)
- worker bridge page (`/remote/worker-bridge`)
- bridge auto-reconnect with exponential backoff for transient disconnects

## Шаг 4 — Remote AI pipeline (in progress)

- Worker runs VAD -> ASR -> translation pipeline in AI mode.
- Worker emits partial/final transcript + translation events back to controller.
- Keep payload contract versioned and stable.

Implemented in this step:
- Worker AI-only guard: `browser_google` mode is rejected in remote worker role.
- Controller -> worker settings sync endpoint:
  - `POST /api/remote/worker/settings/sync`
  - Syncs `translation`, `subtitle_output`, `source_lang`, and enforces `asr.mode=local` on worker.
- Controller proxy endpoints for worker runtime control:
  - `POST /api/remote/worker/runtime/start`
  - `POST /api/remote/worker/runtime/stop`
  - `GET /api/remote/worker/runtime/status`
  - `GET /api/remote/worker/health`
- Controller bridge now supports explicit microphone selection before WebRTC stream start.

## Шаг 5 — Маршрутизация на Controller и UX (in progress)

- Controller routes remote results into existing subtitle router/overlay/export paths.
- Add dashboard controls for:
  - remote role
  - worker address/session/pairing
  - connection status
- Preserve existing local-only UX when remote is disabled.

Implemented in this step:
- Main dashboard recognition mode now enforces AI-only policy for remote worker role:
  - `Browser Speech` option is disabled when effective remote role is `worker`.
  - If config still contains `browser_google` in worker role, UI normalizes it to `local`.
- Desktop launcher now exposes explicit secondary startup profiles:
  - `Remote Controller`
  - `Remote Worker`
  - controller stays lightweight and skips local AI bootstrap by design
  - worker startup keeps LAN bind explicit and stays on the local AI path
- `Remote LAN` panel now auto-refreshes:
  - remote pairing/session state
  - worker runtime status
  - periodic best-effort polling with visibility-aware refresh
- `Remote LAN` panel now supports explicit worker settings sync from controller:
  - `Sync Worker Settings` button
  - worker runtime start now runs settings sync first
- `Remote LAN` panel now includes one-click orchestration:
  - `Prepare Remote Run` performs worker sync + worker runtime start + opens controller bridge

## Правила совместимости

- Local mode остаётся default и полностью работает без remote setup.
- Remote mode — opt-in и не меняет local defaults.
- Не добавляются требования к аккаунтам/облаку/хостингу.
- OBS overlay по-прежнему обслуживается тем же локальным FastAPI приложением.
