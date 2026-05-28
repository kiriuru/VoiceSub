# SST Desktop — WIKI

This guide is written for end users and follows the app interface labels.

---

## Table of Contents

- [0. Version and Updates](#0-version-and-updates)
- [1. Quick Start](#1-quick-start)
- [2. What to Check If Something Is Not Working](#2-what-to-check-if-something-is-not-working)
- [3. Startup Profiles](#3-startup-profiles)
- [4. Recognition](#4-recognition)
- [5. Translation](#5-translation)
- [6. Subtitle Output](#6-subtitle-output)
- [7. Subtitle Style](#7-subtitle-style)
- [8. OBS Closed Captions](#8-obs-closed-captions)
- [9. Recognition Feel (Tuning)](#9-recognition-feel-tuning)
- [10. Advanced Recognition & Diagnostics](#10-advanced-recognition--diagnostics)
- [11. Word replacement (before translation)](#11-word-replacement-before-translation)
- [12. Tools & Data](#12-tools--data)
- [13. Local Parakeet](#13-local-parakeet)
- [14. Web Speech](#14-web-speech)
- [15. Web Speech (Experimental)](#15-web-speech-experimental)
- [16. Help](#16-help)
- [17. Glossary](#17-glossary)

---

## 0. Version and Updates

Update flow:

1. Close the app.
2. Replace `Stream Subtitle Translator.exe`.
3. Launch again.

If something breaks after update:

- use `Repair Runtime`;
- if needed, use `Reset Runtime`;
- CLI options are also available (`--repair`, `--reset-runtime`).

---

## 1. Quick Start

1. Choose a startup profile.
2. In the recognition area, set:
   - `Recognition method`
   - `Recognition language`
   - microphone input
3. In Translation, enable `Translate recognized speech` if needed.
4. Press `Start`.
5. Verify text in preview and `Translated Results`.

---

## 2. What to Check If Something Is Not Working

**No text at all**

- `Start` not pressed;
- wrong microphone selected;
- no microphone permission.

**Source text appears, translation does not**

- `Translate recognized speech` is off;
- no enabled translation lines;
- provider error shown in `Translated Results`.

**OBS shows nothing**

- check overlay URL;
- check `Show the original spoken text` and `Show translated lines`;
- confirm recognition is running.

---

## 3. Startup Profiles

- `Quick Start (Web Speech)`
- `NVIDIA GPU (CUDA)`
- `CPU-only`
- `Remote Controller`
- `Remote Worker`

Important:

- after `Quick Start (Web Speech)`, local AI mode may be unavailable until you start with `NVIDIA GPU (CUDA)` or `CPU-only`;
- in worker mode, browser speech path is not used;
- for remote setup, start worker first, then controller.

---

## 4. Recognition

Main UI items:

- `Recognition method`
  - `Local Parakeet`
  - `Web Speech`
  - `Web Speech (Experimental)`
- `Recognition language`
- `Worker browser (desktop)`
  - `Auto (Google Chrome)`
  - `Google Chrome`
- `Backend ASR provider`
  - `Official EU Parakeet Low Latency`

Notes:

- Web Speech modes use a separate browser worker window;
- in Web Speech mode, mic switching is done through browser permission controls in the address bar.

---

## 5. Translation

### 5.1 Main toggles

- `Translate recognized speech`
- `Reuse translation cache (skip duplicate API calls)`
- `Save translation cache to disk between sessions`

Cache behavior:

- reuse cache skips duplicate provider calls for same text;
- persist cache keeps cache data across app restarts.

### 5.2 Translation Lines

In `Translation Lines`:

- `Target language for new line`
- `Add Line`
- `Remove Selected`
- per-line controls in `Translation line N`:
  - `Enabled`
  - `Target language`
  - `Provider for this line`
  - `Line label` (if shown in your build)

Key points:

- each line can use a different provider;
- duplicate target languages are allowed;
- up to 5 translation lines are supported.

### 5.3 Provider Settings

In `Provider Settings`:

- `Default provider for new lines`
- `Provider settings`
- provider-specific fields:
  - `API key`
  - `Base URL`
  - `Google Apps Script URL`
  - `Endpoint`
  - `Region`
  - `Provider URL`
  - `Model`
  - `Load recommended models`
  - `Pick from list`
  - `Show all returned models`
  - `Custom prompt override`

For LLM providers:

- pick a working model first;
- use short, explicit prompt instructions;
- if the model starts adding extra commentary, simplify the prompt.

### 5.4 Translated Results

`Translated Results` shows:

- successful outputs;
- translation failures (key/model/endpoint/network/timeout issues).

---

## 6. Subtitle Output

### 6.1 Overlay layout preset

`Overlay layout preset` options:

- `Single line`
- `Dual line`
- `Stacked`

Actual behavior:

- `Single line`: all visible items rendered in one row;
- `Dual line`: first item on row 1, remaining items on row 2;
- `Stacked`: each item rendered on its own row.

### 6.2 Compact spacing

`Use tighter overlay spacing`:

- reduces row spacing;
- useful for small OBS subtitle area.

### 6.3 Visibility and cap

- `Show the original spoken text`
- `Show translated lines`
- `Maximum translated lines on screen`

Important:

- you can enable more translation lines than you actually display;
- shown lines follow your current order settings.

### 6.4 Subtitle Timing

`Subtitle Timing` controls:

- `Keep completed source text visible (seconds)`
- `Keep completed translation visible (seconds)`
- `Keep completed source visible while its translation is still visible`
- `Replace the visible block immediately when the next phrase finalizes`

### 6.5 Ordering

Use:

- `Move Up`
- `Move Down`

This order affects:

- dashboard preview;
- overlay payload;
- `First visible line` mode in OBS Closed Captions.

---

## 7. Subtitle Style

Workflow:

1. Choose a preset.
2. Adjust base style.
3. Use per-slot overrides if needed.
4. Save as custom preset.
5. Save config/profile.

Notes:

- style is shared between preview and OBS overlay;
- custom presets can be deleted.

---

## 8. OBS Closed Captions

### 8.1 Connection

In `OBS Closed Captions`:

- `Send captions to OBS Closed Captions`
- `OBS websocket host`
- `Port`
- `Password`
- `Output mode`

### 8.2 Output mode

Options:

- `Disabled`
- `Source live (partials + final)`
- `Source final only`
- `Translation 1` ... `Translation 5`
- `First visible line`

### 8.3 Timing and dedupe

- `Minimum gap between partial sends`
- `Minimum text change before sending another partial`
- `Delay before final replaces the previous text`
- `Clear OBS text after this many milliseconds`
- `Avoid sending identical caption text twice`

### 8.4 Debug mirror

- `Mirror captions into an OBS text source for debugging`
- `Debug text input name`
- `Send partials to the debug text input`

### 8.5 Twitch notes

For viewers:

- captions are toggled with `CC` in Twitch player when available.

Compatibility note:

- Twitch caption ingest is based on CEA-708/EIA-608 compatible caption streams;
- simple text has best compatibility;
- complex Unicode can be inconsistent depending on player/platform path.

---

## 9. Recognition Feel (Tuning)

In `Recognition Feel`:

- `How quickly text appears`
- `How quickly speech is considered finished`
- `How stable or less chatty updates should be`
- `RNNoise noise reduction (experimental)`
- `RNNoise strength`
- `Parakeet latency preset`

After tuning:

- save config/profile;
- restart runtime (`Stop` then `Start`).

---

## 10. Advanced Recognition & Diagnostics

In advanced recognition section:

- `Speech sensitivity mode (VAD)`
- `Latency preset (Parakeet)`
- `Incremental streaming decode (NeMo)`
- `Partial emit mode`
- `Min new words per partial`
- `Partial emit`
- `Min speech`
- `Silence hold`
- `Pause to finalize`
- `Hard max phrase`
- `Minimum text change before updating (chars)`
- `Partial coalescing`
- `Chunk window`
- `Chunk overlap`
- `Ignore very quiet input before ASR`
- `Min RMS`
- `Min voiced ratio`
- `First partial speech`

Recommendation:

- follow “default/safer” hints in UI;
- change one setting at a time.

---

## 11. Word replacement (before translation)

In `Word replacement (before translation)`:

- `Enable word replacement`
- `Include built-in profanity list (English + Russian)`
- `Case-insensitive matching`
- `Whole words only`
- `Word or phrase`
- `Replace with`
- `Add`
- `Remove selected`

Use case:

- fix repeated ASR wording issues before translation/output.

---

## 12. Tools & Data

`Deep Runtime Detail` -> `Runtime Diagnostics` includes:

- latency metrics;
- ASR diagnostics;
- translation diagnostics;
- translation queue/runtime;
- browser worker diagnostics;
- OBS caption diagnostics;
- log location hint.

Lower sections include:

- `Local Config`
- `Profiles`
- diagnostics export

---

## 13. Local Parakeet

UI references:

- `Local Parakeet`
- `Official EU Parakeet Low Latency`

This is the local AI recognition path.

Model reference:

- [NVIDIA Parakeet model card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2)

---

## 14. Web Speech

Worker page includes:

- `Return live partial results`
- `Keep recognition running continuously`
- `Force-complete the current live text if the browser never emits a final result`
- `Start Recognition` / `Stop` / `Save`
- `Live Diagnostics` (status/counters/websocket)
- `Live Partial Text` and `Last Final Text`

---

## 15. Web Speech (Experimental)

Same overall idea as Web Speech, but with experimental start path and fallback to normal start when needed.

---

## 16. Help

`Help` tab provides built-in guidance for:

- startup flow;
- recognition;
- translation;
- subtitles/style;
- OBS;
- tools/diagnostics.

---

## 17. Glossary

- `partial`: in-progress text that may still change.
- `final`: finalized phrase text.
- `slot`: one translation line.
- `overlay`: page used in OBS Browser Source.
- `OBS Closed Captions`: caption stream sent to OBS.

