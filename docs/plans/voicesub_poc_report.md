# VoiceSub Phase 0 — PoC report

**Date:** 2026-06-09 (updated)  
**Status:** Phase 0 **closed** — automated checklist green; manual 30 min soak (Svelte worker, OBS overlap) **operator sign-off received**

## Delivered (roadmap §5 Phase 0)

| Item | Status |
| --- | --- |
| Tauri workspace + 10 crates + thin `src-tauri/` | done |
| Chrome launch parity (flags Appendix A) | done |
| EcoQoS opt-out (Win32 `SetProcessInformation`) | done |
| HTTP `127.0.0.1:8765` + static routes | done |
| WS `/ws/events`, `/ws/asr_worker` | done |
| `external_asr_update` ingest (`partial`/`final`/`is_final`) | done |
| Translation smoke stub → overlay broadcast | done |
| IPC `launch_browser_worker`, `voicesub_version` | done |
| Svelte dashboard (Phase 2 panels) | done |
| Overlay vanilla `/overlay` | done |

## Web Speech worker (Svelte)

| Check | `/google-asr` |
| --- | --- |
| Source | `src-worker/` → `dist-worker/` via `npm run build` |
| WS `/ws/asr_worker` | full SST FSM (ported TS modules + session manager) |
| HTTP integration | `google_asr_served_from_svelte_worker`, `worker_page_and_assets` |
| Automated soak | **pass** (`phase0_soak_checklist_automated`) |
| 30 min soak (OBS overlap) | **pass** (manual, operator) |

## Phase 0 soak checklist

### Automated (CI / `cargo test`)

Test: `voicesub-http/tests/http_ws_smoke.rs::phase0_soak_checklist_automated`

| Step | Roadmap item | Result |
| --- | --- | --- |
| 1 | Dashboard at `http://127.0.0.1:<port>/` | pass |
| 2 | Worker page `/google-asr` | pass |
| 3 | OBS overlay page `/overlay` | pass |
| 4 | `/ws/events` hello + worker partial → live update on events | pass (`transcript_update` or `overlay_update`) |
| 5 | Worker final ingest | pass (WS send, no stall) |

Run locally:

```bash
cd F:\AI\VoiceSub
npm run build
cargo test -p voicesub-http phase0_soak_checklist_automated -- --nocapture
```

### Manual (operator, DoD Phase 0)

**Done** (operator sign-off 2026-06-08):

1. `cargo run -p voicesub-app` (or installed NSIS build when available).
2. Launch Chrome worker via app or open `/google-asr`.
3. Speak — partials/finals on dashboard + `/overlay` in OBS Browser Source.
4. Cover worker with OBS preview **30+ min** — no silent stall.

| Run | Worker UI | Duration | Stall? | Notes |
| --- | --- | --- | --- | --- |
| 1 | Svelte | 30+ min | no | operator sign-off |

## Phase 0 gaps / deferred

- Golden expansion for full browser FSM replay sequences — deferred with Phase 1 formal DoD.
- NSIS installer pipeline — **done** (`build-release.ps1`); public GitHub release — deferred (roadmap §12).
- Parakeet / Remote / Experimental browser — removed from core; archived under `legacy/`.
