# Manual smoke results (non-remote runtime)

Source of truth checklist: `docs/MANUAL_SMOKE_CHECKLIST_NON_REMOTE.md`

Pass metadata:
- date/time (local): `2026-05-08T19:54:43+05:00`
- date/time (utc): `2026-05-08T14:54:43+00:00`
- git commit tested: `e4f949759b7ff17bdad7bca2ecf04b7a94e52160`

Environment:
- OS: Windows 10 (10.0.26200)
- Repo: `F:\AI\stream-sub-translator`
- Python: 3.11.9 (`.\.venv\Scripts\python.exe`)
- Runtime surface: **source/dev** (FastAPI via `python -m backend.run`)
- Selected runtime profile: unknown (not selected in this pass)
- Bind target (observed): `127.0.0.1:8765`

Notes / constraints:
- This pass **does not** touch remote endpoints or remote security.
- Several checklist items require **interactive audio/microphone**, **external browser worker windows**, and/or **OBS**. Those items are recorded as **NOT TESTED** here (with exact manual steps to execute).

Automated verification (recorded):
- `python -m compileall backend desktop tests`
  - result: PASS
- `.\.venv\Scripts\python.exe -m unittest discover -s tests -p "test_*.py"`
  - result: PASS (`Ran 188 tests in 8.193s` → `OK`)

Automated verification (re-run before final report):
- `.\.venv\Scripts\python.exe -m compileall backend desktop tests`
  - result: PASS
- `.\.venv\Scripts\python.exe -m unittest discover -s tests -p "test_*.py"`
  - result: PASS (`Ran 188 tests in 8.452s` → `OK`)
  - note: unittest output includes an expected test-only stack trace from `tests/test_translation_dispatcher.py` forcing a controlled failure path; overall suite status is `OK`.

Local app sanity (HTTP-only; no microphone/OBS automation):
- Sanity was executed against an already-running localhost server on port 8765.
- GET checks:
  - `GET /api/health` → 200 (application/json)
  - `GET /api/runtime/status` → 200 (application/json)
  - `GET /overlay` → 200 (text/html)
  - `GET /google-asr` → 200 (text/html)
  - `GET /google-asr-experimental` → 200 (text/html)
- Update check behavior (by calling `POST /api/runtime/start` with a runtime snapshot, then `POST /api/updates/check`):
  - disabled (`updates.enabled=false`) → 200, `sync.message="Update checks are disabled in settings."`
  - enabled + configured (`updates.enabled=true`, `updates.github_repo="python/cpython"`) → 200, `sync.message="No usable release versions found (scanned 0 releases)."`

## Browser Speech classic (`/google-asr`)

- status: **NOT TESTED**
- exact date/time: `2026-05-08T19:54:43+05:00`
- runtime mode: `asr.mode=browser_google` (expected), desktop/launcher browser worker window
- observed diagnostics:
  - Automated-only verification was not executed for classic worker window behaviors (requires desktop launcher + real browser window).
- any errors/log snippets:
  - none captured (not executed)
- fix recommendation if failed:
  - If classic worker is “dead after restart”, capture:
    - dashboard `GET /api/runtime/status` before/after restart
    - browser worker UI diagnostics panel screenshot / console logs
    - server console logs around `/ws/asr_worker` connect/reconnect

Manual steps to execute:
- Start app and open dashboard (`/`).
- Open worker page (`/google-asr`) via desktop launcher behavior.
  - Confirm: separate browser window, visible address bar, isolated profile directory.
- Start recognition in worker; speak a short phrase.
  - Verify: partial then final appear in dashboard and overlay.
- Stop runtime from dashboard.
  - Verify: overlay clears and status returns to idle.
- Start again.
  - Verify: worker reconnects/continues and transcript flow still works.
- Whether a code fix was applied: **No**

## Browser Speech experimental (`/google-asr-experimental`)

- status: **NOT TESTED**
- exact date/time: `2026-05-08T19:54:43+05:00`
- runtime mode: `asr.mode=browser_google_experimental`
- observed diagnostics:
  - Backend health shows experimental browser speech mode configured (localhost):
    - `GET /api/health`:
      - `asr_provider=browser_google_experimental`
      - `asr_ready=true`
      - message indicates worker window should connect
  - Runtime start (API) enters listening and waits for worker connection:
    - `POST /api/runtime/start` (with `config_payload`) -> `phase=listening`
    - `status_message`: “Waiting for the experimental browser speech worker window to connect.”
- any errors/log snippets:
  - none observed in backend logs during API-driven start/stop (no worker was connected in this pass)
- fix recommendation if failed:
  - If audio-track start fails without fallback, ensure worker UI shows:
    - audio-track start attempts/failures
    - fallback-to-default-start behavior toggles
    - last start error details

Manual steps to execute:
- Repeat the classic flow but with `/google-asr-experimental`.
- Verify:
  - audio-track start path is used when supported
  - fallback to normal recognition happens if audio-track start is rejected (diagnostics in worker UI)
- Whether a code fix was applied: **No**

## Local Parakeet (microphone)

- status: **NOT TESTED**
- exact date/time: `2026-05-08T19:54:43+05:00`
- runtime mode: `asr.mode=local` + Parakeet provider preference (expected)
- observed diagnostics:
  - Not executed (requires microphone capture and local model readiness).
- any errors/log snippets:
  - none captured (not executed)
- fix recommendation if failed:
  - If CPU fallback is unexpected or not surfaced, collect `GET /api/runtime/status` fields:
    - `runtime.degraded_mode`, `fallback_reason`
    - ASR diagnostics: CUDA availability + selected provider

Manual steps to execute:
- Start runtime in local Parakeet mode with a microphone device selected.
- Verify ASR diagnostics:
  - provider/mode correct
  - GPU/CPU fallback status is honest
- Speak:
  - verify partial/final behavior
  - verify stop/start cycles do not break capture or ASR
- Whether a code fix was applied: **No**

## Overlay reconnect

- status: **NOT TESTED**
- exact date/time: `2026-05-08T19:54:43+05:00`
- runtime mode: any non-remote mode; overlay at `/overlay`
- observed diagnostics:
  - Not executed (requires a browser/OBS overlay instance + websocket reconnect behavior observation).
- any errors/log snippets:
  - none captured (not executed)
- fix recommendation if failed:
  - If overlay does not replay latest payload after refresh:
    - capture overlay console logs and `/ws/events` reconnect timeline
    - capture `GET /api/runtime/status` metrics:
      - `ws_events_broadcast_count`, `ws_events_send_failures`, `ws_events_dead_connections_removed`

Manual steps to execute:
- Open `/overlay` in a browser/OBS source.
- Start runtime and speak a short phrase.
- Refresh the overlay page.
  - Verify: latest payload is replayed quickly and continues updating.
- Whether a code fix was applied: **No**

## OBS captions

- status: **NOT TESTED**
- exact date/time: `2026-05-08T19:54:43+05:00`
- runtime mode: any non-remote mode; OBS CC enabled in settings
- observed diagnostics:
  - Backend runtime status shows OBS captions are disabled in current config:
    - `obs_caption_diagnostics.enabled=false`
    - `connection_state=disabled`
- any errors/log snippets:
  - none captured (not executed)
- fix recommendation if failed:
  - If duplicate/stale caption spam occurs on start/stop, collect:
    - `GET /api/runtime/status` `obs_caption_diagnostics.*` and timing fields
    - app logs around OBS websocket connect/reconnect and caption send

Manual steps to execute:
- Enable OBS closed captions output in settings.
- Start runtime, speak a short phrase, then stop runtime.
- Verify:
  - no duplicate/stale caption spam on start/stop
  - restarting runtime does not duplicate caption streams
- Whether a code fix was applied: **No**

## Update check (runtime_start_snapshot protection)

- status: **PASS**
- exact date/time: `2026-05-08T19:51:21+05:00`
- runtime mode: `asr.mode=browser_google_experimental` (non-remote), with `POST /api/runtime/start` supplying `config_payload` (runtime snapshot)
- observed diagnostics:
  - Verified `active_config_source` becomes `runtime_start_snapshot` after runtime start with `config_payload`.
  - After `POST /api/updates/check`, disk persistence was limited to updates metadata only:
    - `user-data/config.json` **did not** persist the runtime-only change `ui.theme='light'` (remained `dark` on disk).
    - Only `updates.last_checked_utc` changed on disk in this run; `updates.latest_known_version` remained empty.
- any errors/log snippets:
  - `POST /api/updates/check` returned HTTP 200.
- fix recommendation if failed:
  - If any non-updates fields are persisted while `active_config_source=runtime_start_snapshot`, treat as a bug in `backend/services/update_service.py` persistence guard.
- Whether a code fix was applied: **No**

