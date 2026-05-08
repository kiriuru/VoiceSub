# Live smoke checklist (non-remote)

Scope: **local Parakeet**, **Browser Speech (classic)**, **Browser Speech Experimental**.

Out of scope: remote controller/worker pairing and any `/remote/*` pages.

## Preconditions

- Run from repo root with the project venv:
  - `.\.venv\Scripts\python.exe`
- OBS not required, but if you test OBS overlay, keep it local-first.
- Confirm default bind is still localhost-only (`127.0.0.1`).

## Baseline sanity (all modes)

- Open dashboard `/` and confirm:
  - Runtime status transitions: `stopped` → `running` → `stopped`
  - Overlay preview updates (visible text matches events)
  - `GET /api/health` returns OK
- Overlay `/overlay`:
  - Auto reconnect works (refresh page / restart runtime)
  - No stale translation glitches (completed translation stays visible during next partials)

## Smoke: local Parakeet

### Start + audio capture

- Start runtime in local Parakeet mode (GPU if available; CPU fallback acceptable but must be surfaced as degraded).
- Select a microphone input.
- Speak: confirm partials appear quickly, and finals appear.

### Translation (optional)

- Enable translation with at least 1 line (`translation_1`) and 1 target language.
- Speak a short phrase:
  - Source final appears
  - Translation arrives and is attached to the correct finalized phrase
  - If a new partial starts, previous completed translation remains visible until the new phrase is finalized

### TTL / relevance edge

- Configure short TTLs (e.g. source TTL < translation TTL, sync disabled) and confirm:
  - After source TTL, translation can remain visible as translation-only until translation TTL
  - Late translation can still appear as translation-only if source TTL already elapsed but translation TTL still valid

## Smoke: Browser Speech (classic)

### Worker window invariants (desktop mode)

- Launch Browser Speech worker page (`/google-asr`) using the launcher behavior.
- Confirm:
  - It opens as a **separate browser window** with a **visible address bar**
  - It uses an **isolated browser profile directory** (no cross-talk with your normal browser profile)
  - Microphone selection is possible via the browser permission UI in the address bar

### End-to-end transcript flow

- With runtime started in Browser Speech mode:
  - Speak into the browser worker
  - Confirm partial and final transcript events reach the dashboard/overlay
  - Stop runtime; confirm overlay clears and worker status updates

## Smoke: Browser Speech Experimental

- Repeat the classic Browser Speech steps, but using `/google-asr-experimental`.
- Confirm the same invariants:
  - Separate window, visible address bar, isolated profile directory
  - Events reach dashboard/overlay and lifecycle behaves on stop/start

## Restart regression (critical)

For each of the 3 modes above:

- Start runtime → speak → stop runtime → start again → speak again
- Confirm:
  - No “silent failure” after restart (translation dispatcher continues working after stop/start)
  - No duplicate tasks keep running after stop
  - Overlay continues receiving payload updates after restart

