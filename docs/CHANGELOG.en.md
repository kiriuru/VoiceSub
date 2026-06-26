# VoiceSub / SST Desktop Changelog

<p align="center"><a href="./CHANGELOG.en.md">English</a> • <a href="./CHANGELOG.md">Русский</a></p>

Unified change history for the desktop line: **VoiceSub** `0.5.5` (current line), **VoiceSub** `0.5.4`, **VoiceSub** `0.5.3`, **VoiceSub** `0.5.2`, **VoiceSub** `0.5.1`, **VoiceSub** `0.5.0` (first release of the new line, baseline — SST `0.4.4`), and **SST Desktop** `0.4.4` and below.



**Entry format (as in [GitHub Release v0.2.9.2](https://github.com/kiriuru/stream_sub_translator/releases/tag/v0.2.9.2)):** one sentence about the version; “what’s included” bullets — facts only; for desktop-exe / installer — a “release format” block (delivery structure, without listing old profiles as new features).

## 0.5.5

Patch release. `PROJECT_VERSION` — **0.5.5**; `config_version` **8** (unchanged). Focus: further reduction of Tauri IPC and CPU load on the subtitle/ASR hot path — dashboard `overlay_update` IPC coalescing (OBS WebSocket unchanged), decoupled ingest vs fanout, lock-free WS event sequencing, bus lag metrics, and safer lag-resync debouncing. **Contracts unchanged:** OBS `/ws/events` still receives every `overlay_update`; subtitle lifecycle, overlay payload shape, browser worker `/ws/asr_worker` protocol.

### Runtime — Tauri IPC pump (CPU / WebView2)

- **`src-tauri/src/ipc_pump.rs`** (new) — dedicated bus→IPC pump: **trailing-edge coalescing** of `overlay_update` to the **main dashboard only** (default 90 ms, env `VOICESUB_OVERLAY_IPC_MIN_INTERVAL_MS`; `0` = disabled). OBS overlay and `/ws/events` clients still receive every frame.
- **`runtime_update` / `translation_update`** flush any pending coalesced overlay immediately.
- On `broadcast::RecvError::Lagged`: records metrics, **debounces** full snapshot resync (200 ms), runs resync in a **background task** so the pump keeps processing events and overlay timers.
- Flushes pending overlay on bus shutdown.

### Runtime — ingest vs fanout

- **`transcript_controller.rs`** — subtitle lifecycle runs **before** WS/IPC fanout; partial `transcript_update` broadcast is **async** (`tokio::spawn`) so ingest is not blocked on enrich/broadcast.
- Final transcripts still await `transcript_update` publish before translation `submit_final` ordering is preserved via `handle_transcript` await.

### Runtime — WS enrich + diagnostics

- **`voicesub-ws/event_sequence.rs`** — `global_sequence` via `AtomicU64`; hot-path enrich no longer takes an outer `Mutex`.
- **`RuntimeMetricsCollector`** — `event_bus_consumer_lagged_total`, `event_bus_consumer_lagged_messages_skipped`, `overlay_ipc_coalesced_suppressed` exposed in `/api/runtime/status` metrics.

### Dashboard

- **`App.svelte`** — skips 4 s HTTP runtime poll when Tauri IPC is connected; adds 30 s safety-net poll when IPC is active.

### Documentation

- **`docs/TECHNICAL_ARCHITECTURE.en.md`** / **`docs/TECHNICAL_ARCHITECTURE.md`** — IPC pump, overlay IPC coalescing env, ingest ordering.

### Env (new / affected in 0.5.5)

| Variable | Purpose |
| --- | --- |
| `VOICESUB_OVERLAY_IPC_MIN_INTERVAL_MS` | Trailing-edge coalesce for dashboard `overlay_update` IPC only (default **90**; **`0`** = disabled; OBS WS unaffected) |

### Desktop release format

| Item | Value |
| --- | --- |
| Installer | **`VoiceSub_0.5.5_x64-setup.exe`** |
| Build | `build-release-msi.bat` → `build-release.ps1` → `F:\AI\VoiceSub - release\v0.5.5\` |

### Migration notes

- No `config_version` change — configs from 0.5.4 load as-is.
- Dashboard live preview may update overlay IPC at most ~90 ms slower during rapid partial speech; OBS overlay timing **unchanged**.

## 0.5.4

Patch release. `PROJECT_VERSION` — **0.5.4**; `config_version` **8** (unchanged). Focus: lower CPU/WS/IPC load on subtitle and ASR hot paths, per-window Tauri `runtime-event` routing, reduced system impact of the Chrome worker, TTS/Twitch resilience to network failures, Rust TTS pipeline reliability (prefetch, config I/O, audio-chunk ordering), **full TTS module cleanup** (removal of JS queue pump and deprecated IPC), Twitch chat log fix in the TTS module, ingest latency diagnostics with full logging, HTTP/WS fanout fixes. **WebSocket:** live subtitles — `overlay_update` only; ASR — `transcript_update` only (removed unused duplicate `transcript_segment_event`). `subtitle_payload_update` — **Tauri IPC snapshot only** (not published on `/ws/events`; WS replay on connect — `runtime_update` + `overlay_update`). Overlay/ASR payload **unchanged**. Browser worker `/ws/asr_worker` **unchanged** (protocol); only process priority class and orphan PID reap on start were changed.

### Runtime — ASR WS fanout (CPU)

- **`transcript_controller.rs`** — removed duplicate broadcast `transcript_segment_event` (same payload as `transcript_update`); no consumers in tree (dashboard, overlay, TTS — `transcript_update` only).
- **Effect:** ~−50% WS broadcast on ASR partial/final path.

### Runtime — subtitle WS fanout (CPU / OBS)

- **`service.rs`** — removed duplicate live broadcast `subtitle_payload_update` on every publish; OBS overlay and dashboard preview receive live frames via `overlay_update` (`OverlayBroadcaster` + dedupe).
- **`HttpState::flush_overlay_presentations_to_clients`** — flush `overlay_update` only (no second identical message).
- **`runtime_state_snapshot`** — `subtitle` field read from `last_subtitle_payload` (no `MutexGuard` across `.await` in Tauri IPC).
- **Effect:** ~−50% WS broadcast/min on subtitle path during continuous speech (confirmed in logs); lifecycle/presentation **unchanged**.

### Runtime — browser ingest hot path

- **`browser_speech_source.rs`** — sync `accept_update` + async `process_ingest_work`: ingest mutex **not** held during `handle_event` / WS / subtitle actor.
- Removed full `config.toml` clone on every partial; `source_lang` read pointwise from snapshot.
- **`transcript_controller.rs`** — `source_text_replacement` parsed from section without cloning entire config.

### Runtime — Tauri IPC fanout & system impact

- **`src-tauri/src/event_routing.rs`** (new) + **`lib.rs`** — `runtime-event` routed via `emit_to(label, …)`: main window receives all types, TTS window — only `twitch_*` / `runtime_update` / `runtime_status` / `ui_config_sync`. High-frequency `transcript_update` / `overlay_update` no longer floods the TTS webview IPC channel. Payload passed by reference (no deep-clone per event).
- **`lib.rs`** — on `broadcast::RecvError::Lagged`, IPC pool re-syncs dashboard from snapshot (`snapshot_to_envelopes`) instead of silent drop. Snapshot includes `twitch_connection` (status), so TTS window connection UI recovers after lag; chat messages are ephemeral and not replayed.
- **`transcript_controller.rs`** — leading-edge coalescing of partial **`transcript_update` only** (default 90 ms, env `VOICESUB_TRANSCRIPT_PARTIAL_MIN_INTERVAL_MS`; new phrase/`sequence` and all finals — no delay). Subtitle lifecycle, `overlay_update`, OBS overlay and dashboard preview **see every partial**; only the redundant raw ASR channel is coalesced (Overview “Partial”, not live render).
- **`overlay_broadcaster.rs`** — `dedupe_signature` (deep-clone + key sort + serialize) computed only for stable states; skipped on `partial_only` / `completed_with_partial`; partial between two identical `completed_only` resets dedupe anchor.
- **`voicesub-ws/events.rs`** — `enqueue_to_client` accepts `message_type` from caller; removed repeated JSON parsing per WS client.
- **`service.rs`** — `SubtitlePayloadForwarder`: TTS listener called on a dedicated ordered thread (`voicesub-subtitle-payload-forward`), not inside subtitle actor publish loop. On queue overflow, oldest **non-speakable** frame dropped (partial-only / active); speakable (`completed_*`) preserved; listener panic isolated (`catch_unwind`) — forwarder thread does not crash.
- **Tests:** `event_routing` (per-window routing + snapshot envelopes), `transcript_controller` (`partial_throttle_*`), `overlay_broadcaster` (`partial_frame_clears_completed_dedupe_anchor`).

### Browser worker — priority and orphan processes

- **`launcher.rs`** — Chrome worker launched with `ABOVE_NORMAL_PRIORITY_CLASS` instead of `HIGH_PRIORITY_CLASS`: ASR stays responsive (anti-throttling + EcoQoS opt-out preserved), but does not evict other programs from CPU. Fallback to normal on `ERROR_ACCESS_DENIED`.
- **`orphan_guard.rs`** (new) — worker PID saved to `user-data/browser-worker.pid` on launch (`http/runtime.rs` **and** Tauri IPC `launch_browser_worker` → `service.rs`), removed on graceful stop; `RuntimeService::start` kills orphan worker from previous *crashed* session only if PID still belongs to Chromium process (`chrome.exe`/`msedge.exe`) — PID reuse protection.
- **Tests:** `orphan_guard` (PID roundtrip, image guard, tasklist CSV parse).

### Browser worker — overlap ASR handoff

- **`overlap-logic.ts` / `recognition-handlers.ts` / `session-manager.ts`** — buddy-slot pre-start invoked at segment boundary and on **suppressed** (duplicate / late-forced) final, not only on success — idempotent (guard `recognitionOverlapPrestarted`), not on teardown path. Restores overlap if previous pre-start failed.
- **`overlap-logic.ts`** — pre-start retry (`scheduleBuddyPrestartRetry`) stores timer handle in state, bound to `recognitionGenerationId`, cancelled in `clearAllTimersInternal` / on new slot pair — timer can no longer call `buddyRec.start()` on completed session.
- **`overlap-logic.ts` / watchdog** — `shouldRestartStalledOverlapHandoff`: if warming buddy promoted to active but its `onstart` did not fire within `buddyGhostActiveMicMs`, watchdog does fast rearm instead of waiting for visible-idle (~30s).
- **Behavior:** buddy pre-start — committed segment only; time/speech-based early prestart not used (slightly higher latency on first phrase possible — by design).
- **`recognition-handlers.ts`** — removed unreachable duplicate guard in `case "aborted"` (overlap already filtered in `applyRecognitionError`).
- **Tests:** `overlap-logic.test.ts` (retry timer cleanup, stalled handoff, suppressed-final prestart).

### Browser worker — long-segment flush (post-monologue fragmentation)

- **`long-segment-flush-logic.ts`** (new) — after committed final (natural or forced), if partial/final segment peak ≥ **200** chars, worker flushes “clogged” Web Speech `results` buffer: in `native_continuous` — `recognition.flush` (`long_segment_flush`, delay like `session_cycle` ~150 ms); in overlap (`continuous=false`) — `stop()` on active slot only **after** `preStartNextOverlapInstance` for race-safe handoff to buddy.
- **`recognition-handlers.ts` / `session-manager.ts`** — track `currentSegmentPeakPartialChars` on interim; no flush on partial or with pending restart/stopping.
- **`recognition-lifecycle.ts`** — `requestRecognitionFlush`; `transitionToStopping` for `long_segment_flush` does not call `forceFinalizeOnInterruption` (segment already committed).
- **Symptom (before fix):** after long monologue (300–400+ chars in one final) next speech consistently split into short finals (tens of chars) — confirmed in `pipeline-trace.jsonl` (`asr_ingest_final_published`).
- **Invariants unchanged:** overlap prestart on final only, force-finalize idle 1600 ms, dedup partial/final, subtitle lifecycle.
- **Tests:** `long-segment-flush-logic.test.ts`.

### Diagnostics — pipeline ingest latency

- **`trace.rs`** + **`transcript_controller`** — `ingest_latency_ms` field in `pipeline-trace.jsonl` when `logging.full_enabled` (or deep diagnostics); runtime collector metrics.
- **Test:** `asr_ingest_published_includes_latency_when_provided`.

### Timestamp format (diagnostics / subtitle lifecycle) — ⚠ change

- **`voicesub-types/time.rs`** (new) — RFC 3339 UTC helper (`utc_now_rfc3339`, `epoch_secs_to_rfc3339`) without external crates.
- **`voicesub-logging` (`session.rs`, `jsonl_trace.rs`)** and **`voicesub-subtitle/lifecycle.rs`** — fields `timestamp_utc`, `finalized_at_utc`, `completed_expires_at_utc` are now **RFC 3339 strings** (`2026-06-21T07:01:00Z`) instead of numeric epoch seconds (`"1781806869"`). Payload structure (keys) unchanged; only value format changed.
- **Impact:** overlay/dashboard do not parse these fields as numbers — subtitle render unaffected. External scripts/diagnostics expecting epoch seconds should accept both forms. `tools/_analyze_tts_session.py` updated (`ts()` accepts epoch and RFC 3339).

### TTS — network resilience + speech settings

- **`upstream_retry.rs`** — shared retry helper (3 attempts, transport + HTTP 5xx/429/408).
- **`google_fetch.rs`** — retries, connect/read timeouts on reqwest client.
- **`python_runtime.rs`** — same retry helper for embedded fetch.
- **`http/tts_proxy.rs`** — delegates to `fetch_google_tts_browser` (no duplicate HTTP client).
- **`config.rs`** — `normalize_speech_settings()` clamp `min_chars` 1–32, `max_queue_items` 1–64.
- **Tests:** `subtitle_speech.rs`, `subtitle_speech_service.rs` (`min_chars`, saved settings).

### TTS — pipeline, prefetch & config I/O

- **`google_fetch.rs`** — `assemble_ordered_chunks`: long TTS text assembled in original order after parallel fetch (`JoinSet` returned chunks by completion, not index). Fetch concurrency limited by semaphore (`GOOGLE_TTS_PREFETCH_MAX_CONCURRENCY = 3`) — does not burst many parallel Google TTS requests (429/503). Tests: `assemble_ordered_chunks_*`.
- **`channel_orchestrator.rs`** — guard against parallel duplicate prefetch (`in_flight`); prefetch wait via `Notify` instead of busy-poll; Twitch channel uses `effective_speech_rate` / `effective_speech_volume`; `check_stuck` / `clear` / `set_enabled(false)` symmetrically cancel `completion_waiter`.
- **`config.rs`** — in-memory cache for hot-path reads (invalidation by file mtime — external `config.toml` edits picked up without restart); atomic save (temp + rename); backup of corrupted `config.toml` instead of silent overwrite with defaults. Tests: legacy migrate, corrupt backup, temp cleanup, mtime invalidation.
- **`ipc.rs`** — `speech_queue_item_id`: monotonic counter inside millisecond bucket (id collisions on burst enqueue).
- **`service.rs`** — adaptive queue cap reads `max_queue_items` from normalized speech settings, not hardcoded `8`.

### TTS — subtitle speech planner

- **`subtitle_speech.rs`** — speech planning for `completed_with_partial` (previously `completed_only` only); `sequence` from `completed_sequence`; partial source line skipped until phrase finalized. Tests: `subtitle_speech_service.rs`.

### TTS module — Twitch chat log

- **`twitch-chat-log.ts`** (new) — dedupe incoming chat events by Twitch `id` / `event_sequence` before prepend to log.
- **`TwitchPanel.svelte`** — unique `logKey` for keyed `{#each}` when IRC `id` empty/repeating (otherwise Svelte freezes after first line).
- **`twitch-chat-log.ts`** — dedupe fallback without stable id/seq/`created_at_ms` uses unique per-row key (`logKey`) instead of `user|channel|text` — two different messages with same text no longer collapse.
- **`runtime-events.ts`** — new `runtime-event` listener connected **before** unsubscribing previous: transient `listen` failure does not leave window without IPC channel.
- **`App.svelte`** — generation guard in `connectRuntimeEvents` (cancel stale async connect); auto-reconnect IPC channel in 3s runtime timer if it dropped.
- **Tests:** `twitch-chat-log.test.ts` (fallback dedupe), `runtime-events.test.ts` (twitch_connection replay).

### TTS — volume 150% and numeric labels (Speech / Twitch)

- **`voicesub-audio/playback.rs`** — `SPEECH_VOLUME_MAX = 1.5`, `clamp_speech_volume()`; native playback via `rodio` `amplify()` (0–150%).
- **`voicesub-tts/config.rs`** — `normalize_tts_config` clamp root `speech_volume` and twitch override (`>= 0`); **`service.rs`** — `update_voice_settings`.
- **`src-tts`** — volume sliders `max="1.5"` (`App.svelte`, `TwitchPanel.svelte`); `speech-playback-policy.ts` (`PLAYBACK_VOLUME_MAX`, `clampSpeechVolume`); `playback-format.ts` (`formatSpeechVolume` → `150%`).
- **Twitch advanced** — rate/volume override with live numbers like Speech tab (`1.25×`, `85%`); rate slider — `oninput` + `onchange` (number updates while dragging).
- **Tests:** `clamp_speech_volume_allows_one_hundred_fifty_percent`, `normalize_clamps_speech_volume_to_one_hundred_fifty_percent`, `playback-format.test.ts`, `apply_speech_volume_boosts_quiet_pcm_above_unity`.
- **Fix (playback):** native path — decode → `apply_speech_volume_to_pcm` (f32) instead of rodio `i16` `amplify()`; **>100%** — compression + makeup gain + brick-wall limit (not linear `×1.5` on already clipped TTS); browser — same algorithm on `AudioBuffer`; UI — debounced save while dragging slider.

### TTS module — cleanup JS queue & deprecated IPC

- **Sample test** — Speak test button in `/tts` no longer uses JS `SpeechEngine`; IPC **`tts_speak_sample`** → `TtsSpeechPipeline::enqueue_speech_test` → `ChannelOrchestrator` (same hot path as subtitle/twitch). Activity log — event `tts-speech-activity`; status — `playback-finished` on `speech` channel. Sample test **does not require** `runtime_active` (unlike subtitle path).
- **Removed frontend modules:** `speech-engine.ts` (~965 lines), `browser-audio-output.ts`, `twitch-replacements.ts`, `theme.ts`, `source-text-replacement.ts`.
- **`google-tts.ts`** — kept warmup, URL builder and chunking; removed WebAudio/HTMLAudio playback (`playGoogleTts`, prefetch-to-speaker path ~200 lines).
- **`audio-player.ts`** — mode helpers only (`isNativePlaybackMode` / `isSonicPlaybackMode`); `NativeAudioPlayer` and JS pump removed.
- **`speech-playback-policy.ts`** — clamp/format helpers only; queue-depth rate boost (duplicate of Rust `playback_policy.rs`) removed.
- **`tts-ipc.ts`** — removed `planSubtitleSpeech`, `openTtsWindow`, `syncSourceTextReplacement`, `channelEnqueue` / `channelBeginNext` / `channelFinish` / `channelSnapshot`; added `speakSample`.
- **`translation-lines.ts`** — removed legacy test-target API (`buildSpeakTargets`, localStorage target id).
- **`App.svelte`** — no `SpeechEngine`; `clearSpeechChannels` via `tts_channel_clear`; removed `applyDashboardConfigPayload`.
- **`TwitchPanel.svelte`** — unused prop `onConnectionChange`.
- **Rust `voicesub-tts`** — `enqueue_speech_test`, `wake_after_enqueue`; removed dead `TtsModuleService` methods (OAuth wrappers, `save_config`, `list_output_devices`, `queue_clear*`, …), `SpeechQueueState::Paused` / `pause` / `resume`; `channel_queue` uses `voicesub_audio::CHANNEL_*`; narrowed re-exports in `lib.rs`.
- **⚠ Breaking (Tauri IPC):** removed **`tts_enqueue`**, **`tts_plan_subtitle_speech`**, **`tts_channel_enqueue`**, **`tts_channel_begin_next`**, **`tts_channel_finish`**, **`tts_channel_snapshot`**, **`tts_sync_source_text_replacement`**. Subtitle speech planned only in Rust (`speech_pipeline::handle_subtitle_payload`). Speech queue clear — **`tts_channel_clear`**; recovery — **`tts_channel_force_idle`**.
- **Tests:** `speech_pipeline::speech_test_*`, `google-tts.test.ts`, `speech-playback-policy.test.ts`; `cargo test --workspace`, `npm run test:frontend`.

### Twitch — chat filters (mentions, digits, links)

- **`lang.rs`** — `normalize_twitch_mentions()` for TTS (`@nick` → `nick`, text preserved); clean/detection path — full `strip_twitch_mentions`.
- **`pipeline.rs`** — TTS `speak_text` and UI `clean_text` from different branches; `has_meaningful_linguistic_content(text, strip_links)` — link strip/reject only when `strip_links=true`.
- **`emoji.rs`** — `strip_invisible_chat_characters` (U+034F, U+3164, `\p{Cf}`, …) before filters; **`lang.rs`** — `is_digit_group_token` (`500&100`, `500$100`, etc.).
- **`symbols.rs`** — `&`/`$` between digits → space; URL params not broken.
- **`links.rs`** — markers `github.com/` etc.; **`strip_leading_speaker_label`** — does not treat `https:` as speaker label.
- **Tests:** `voicesub-twitch` (mentions, digit groups, link-only with `strip_links=false`).

### Twitch — IRC reconnect hardening

- **`irc.rs`** — TLS EOF / `close_notify` → `Disconnected` (retry), not fatal `Error`; jitter on reconnect backoff; `TCP_NODELAY`; pong failures → disconnect.
- **`service.rs`** — UI cleanup if reconnect loop ended in `connecting`.
- **`error.rs`** — `is_retryable()` for auth vs network.

### Runtime — HTTP/WS path fixes

- **`voicesub-ws/publisher.rs`** — sync-fallback (`broadcast_*_now` outside Tokio runtime): removed second `RuntimeEventBus::publish` in worker thread; Tauri `runtime-event` no longer receives duplicate overlay/translation frame. Test: `broadcast_now_publishes_event_bus_exactly_once_outside_tokio`.
- **`http/ui_sync.rs`** — `POST /api/ui/sync` via `ws_publisher.broadcast_channel` instead of direct `EventsHub::broadcast`: `ui_config_sync` gets `event_sequence`/`created_at_ms` enrichment and enters `RuntimeEventBus` (Tauri IPC), like other channels.
- **`voicesub-ws/events.rs`** — removed dead `subtitle_payload_update` from WS replay on connect (type no longer published on hub); `replay_last` — one `clients.read()` per call, serialization error does not send empty Text frame to client. Tests: `subtitle_payload_update_not_replayable`, `replay_last_absent_type_returns_true`.
- **`service.rs`** — `SessionLogManager` created once in `build()` and reused in `http_state()` (not recreated on every call).
- **`http/openai.rs`** — `usable_models` documented as alias `list_models`; `OpenAiModelsRequest` fields marked `#[allow(dead_code)]` (API contract, static model list).

### Config — deprecated lifecycle timing keys

- **`pause_to_finalize_ms`** / **`finalization_hold_ms`** and **`hard_max_phrase_ms`** / **`max_segment_ms`** marked **deprecated** in `config-normalize.ts`, `voicesub-config/normalize.rs`, `data/config.schema.json` (`"deprecated": true` + description), `types.ts`, `lifecycle.rs`.
- Keys **preserved** on normalize/load for old configs and trace parity, but **do not affect runtime** (subtitle FSM and worker do not read them).
- **Current** forced-final idle setting — `asr.browser.force_finalization_timeout_ms` (Web Speech worker window UI).
- **`data/config.example.json`** — deprecated keys removed from example (normalize supplies on import).

### Dashboard — advanced Web Speech settings

- **New defaults** (live-stream tuning, synced `webspeech-advanced-defaults.ts` → `config-normalize.ts`, `defaults.rs`, `worker-defaults.ts`, worker session fallbacks, `config.schema.json`):

| Key | Was | Now |
| --- | ---: | ---: |
| `force_final_min_chars` | 3 | **8** |
| `force_final_min_stable_ms` | 700 | **750** |
| `normal_restart_delay_ms` | 350 | **150** |
| `no_speech_restart_delay_ms` | 350 | **150** |
| `stuck_stopping_timeout_ms` | 2500 | **2000** |
| `network_reconnect_initial_ms` | 1000 | **500** |
| `prepare_cycle_before_ms` | 15000 | **30000** |
| other advanced | — | unchanged |

- **`WebSpeechAdvancedSettings.svelte`** — each field has **`!`** button (`FieldHelpButton.svelte`) with localized impact description (en, ru, ja, ko, zh); popover on click + `title` on hover.
- **i18n:** `settings.webspeech.advanced.<field>.help`, `settings.webspeech.advanced.help_trigger` in `scripts/export-i18n.mjs`.

### Dashboard / frontend — UI cleanup (conservative cleanup)

- **Formatting** — normalized double blank lines in `SettingsPanel.svelte`, `SubtitlesPanel.svelte`, `TranslationPanel.svelte`, `DashboardPanels.svelte` (logic unchanged).
- **`SettingsPanel.svelte`** — removed duplicate `settings.fonts.hint`; fonts section: `settings.fonts.eyebrow` / `settings.fonts.title`; inline `margin-top` → `.section-heading--spaced` (`global.css`).
- **`WebSpeechAdvancedSettings.svelte`** — same spacing classes instead of inline `style`.
- **`StylePanel.svelte`** — one heading per base/slots section (no duplicate eyebrow); removed extra `settings.fonts.hint` at bottom; preset delete button → `style.custom_preset.delete`.
- **`ReplacementPanel.svelte`** — unified `$: tr = …` pattern.
- **`ToolsPanel.svelte`** — removed extra `refreshLists()` after export diagnostics; under runtime block — `tools.runtime.note`.
- **`SubtitlesPanel.svelte`** — removed local `.checkbox-row` (uses global); `subtitles.display_order` text aligned with UI reorder (up/down), not “comma-separated ids”.
- **`MoreSettingsHub.svelte`**, **`SubtitlesSettingsHub.svelte`** — `.muted--flush` instead of inline `margin: 0`.
- **i18n prune** — ~**27** orphan keys × 5 locales from dashboard bundle (`worker.description`, `worker.microphone.*`, `worker.metric.*` except `websocket`, `header.*`, `document.title.dashboard`, unused `tools.source_replacement.*`); catalog **736** keys × 5 locales (was ~763). Script: extended `scripts/prune-asr-orphan-i18n.mjs`.
- **i18n (new keys):** `settings.fonts.title`, `settings.fonts.eyebrow`, `style.custom_preset.delete`; **`help.tools.body`** — word replacement and advanced ASR referenced in More → Word Replace and Settings (not Tools & Data).
- **TTS locales** — removed 8 orphan keys from each `tts-{en,ru,ja,ko,zh}.json` (`browse_audio`, `playback_mode.browser`, `speaking_twitch`, …).
- **`src/lib/i18n/index.ts`** — removed unused export `tStore`; **`ui-config-sync.ts`** — removed `mergeUiConfigPatch`; **`style-presets.ts`** — `presetDescription` private.
- **Worker** — fallback `appVersion` in `worker-ui.svelte.ts` synced with **0.5.4**.
- **Overlay** — removed unused script `dynamic-locales.js` from `overlay.html` (overlay keys already in `locales-bundle.js`); copy step removed from `scripts/build-locale-bundle.mjs`; dead `debugEntries` buffer in `overlay.js`.
- **Contracts unchanged:** config payload, subtitle/overlay lifecycle, HTTP/WS; deprecated `pause_to_finalize_ms` / `hard_max_phrase_ms` sync untouched (except deprecated **TTS** IPC removal — see § TTS module cleanup).

### Documentation

- **`docs/TECHNICAL_ARCHITECTURE.en.md`** / **`docs/TECHNICAL_ARCHITECTURE.md`** — full sync with 0.5.4 code: Material 3 navigation, `POST /api/ui/sync`, Tauri IPC (`tts_speak_sample`, JS queue IPC removal), TTS pipeline (`upstream_retry`, twitch-chat-log), **volume 0–150%**, Twitch chat filters (mentions, `strip_links`, invisible chars), env vars, removed nonexistent `xtask`; **§12** advanced Web Speech defaults + field help UI; **§14** deprecated `pause_to_finalize_ms` / `hard_max_phrase_ms`.
- **`docs/WIKI.en.md`** / **`docs/WIKI.ru.md`** — advanced Web Speech settings; TTS planner/sample test via Rust orchestrator.

### Env (new / affected in 0.5.4)

| Variable | Purpose |
| --- | --- |
| `VOICESUB_TRANSCRIPT_PARTIAL_MIN_INTERVAL_MS` | Minimum interval for partial `transcript_update` (default **90**; `0` = no coalescing). Does not affect `overlay_update`. |
| `VOICESUB_BROWSER_AFFINITY` / `VOICESUB_BROWSER_AFFINITY_MASK` / `VOICESUB_BROWSER_AFFINITY_EXCLUDE_LOW` | Worker CPU affinity (unchanged; documented in arch §12). |

### Release

| Surface | Value |
| --- | --- |
| Installer | **`VoiceSub_0.5.4_x64-setup.exe`** |
| Build | `build-release-msi.bat` → `build-release.ps1` → `F:\AI\VoiceSub - release\v0.5.4\` |

---
## 0.5.3

Patch release. `PROJECT_VERSION` — **0.5.3**; `config_version` **8** (unchanged). Relative to [v0.5.2](https://github.com/kiriuru/VoiceSub/releases/tag/v0.5.2): dashboard navigation redesign (Material 3 shell), loopback API auth completion on all trusted surfaces, GitHub update check fixes for migrated SST configs, browser worker UI update, documentation consolidation after port completion, dead code / obsolete i18n cleanup, Rust edition 2024 + CI. HTTP/WebSocket contracts for **OBS overlay** (`/overlay`) and **browser worker** (`/ws/asr_worker`) **unchanged** (payload).

### HTTP — loopback API auth (completion)

- **`loopback_auth.rs`** — per-session `x-voicesub-token` middleware for `/api/*`; constant-time compare; public exceptions: `GET /live`, static assets.
- **HTML injection** — trusted pages (dashboard, `/google-asr`, `/tts`) receive `window.__VOICESUB_API_TOKEN__` on serve; Tauri IPC `get_loopback_api_token`.
- **Shared client** — `src/lib/loopback-api.ts` + `loopback-api-client.ts`; worker and TTS import same fetch wrapper.
- **`ui-config-sync`** — debounced POST to `/api/settings/save` only with valid token.
- **OAuth callback** (Twitch TTS) — init loopback token before API calls.
- **Tests:** `loopback-api.test.ts`, `loopback-api-bootstrap.test.ts`, `runtime_lifecycle` HTML injection, `authed_api.rs` helper for integration smoke.

### HTTP — background tasks diagnostics

- **`BackgroundTaskRegistry`** — snapshot `http_server`, `runtime_heartbeat`, `startup_check` in runtime metrics/diagnostics.
- Startup update check marks registry and clears flag on completion.

### Updates — GitHub release check

- **SST config migration** — `normalize_updates_config`: for `github_repo` = `kiriuru/VoiceSub` / legacy slug and `enabled=false` without explicit user opt-out, enables update check (SST disabled updates by default).
- **Stale cache bypass** — if `latest_known_version` < `PROJECT_VERSION`, interval gate does not block new poll (after local upgrade).
- **Dashboard** — `postClientLog` on `checkUpdates` error on boot; structured `tracing::info` when `updates.enabled=false`.
- **Tests:** `normalize_updates_*` in `voicesub-config`; version compare in `update_service.rs`.

### Dashboard — Material 3 navigation shell

- **`navigation.ts`** — primary destinations: Live, Translation, Subtitles, OBS, Modules, More; hub sub-screens for More and Subtitles.
- **Standard layout** — `StandardShell` + `NavRail` + `TopAppBar` + `RuntimeBar`; hub panels `MoreSettingsHub`, `SubtitlesSettingsHub`.
- **Compact layout** — `CompactShell` + `BottomNav`; same destinations, phone-style UX.
- **New components** — `SaveSnackbar` (auto-dismiss save/restart hints), `ScrollToTopFab`, `RuntimeDetailsSheet`, `RuntimeStatusStrip`, `RuntimeMiniStrip`, `SubtitleOutputPreview`, `ModulesPanel`, `PanelSectionNav`, `PanelTopNavLayout`.
- **Removed** — `TabNav.svelte`, `AppChrome.svelte` (replaced by shell components).
- **`OverviewSection`** — reworked live preview + runtime controls for new navigation.
- **Styles** — `shell.css`, `mica-shell.css`, updated `tokens.css`, `surfaces.css`, `compact-layout.css`, `bento.css`.
- **i18n** — `nav.*` keys for rail/bottom nav (en, ru, ja, ko, zh).
- **Tests:** `navigation.test.ts`, `panel-sections.test.ts`, `runtime-status.test.ts`, `scroll-to-top.test.ts`, `shell-platform.test.ts`.

### Browser Speech worker — UI

- **`WorkerApp.svelte`** — compact shell, aligned with dashboard theme tokens.
- **CSS** — `worker-shell.css` removed; styles in `worker.css`.
- **`ui-theme.ts`** — simplified; palette via shared tokens.
- **Loopback** — `initLoopbackApiToken` on worker boot.

### TTS module — loopback + styling

- Shared `loopback-api-client`; token init on mount.
- CSS refresh (`tts-module.css`); minor `TwitchPanel`, trace helper fixes.

### TTS — Twitch IRC auto-reconnect

- **`run_session_with_reconnect`** (`voicesub-twitch/irc.rs`) — after IRC/TLS break or stream close, automatically reconnects with exponential backoff **1→30 s**; backoff resets after successful JOIN.
- **Non-retryable** — OAuth/nick errors and `InvalidSettings` stop loop with `error` status; manual **Disconnect** — via `stop_rx` + task abort.
- **UI** — on reconnect status returns to `connecting` (Disconnect button available); WS payload `twitch_connection_update` unchanged.
- **Tests:** `TwitchError::is_retryable()` (auth vs network).

### OBS overlay — logging hardening (follow-up)

- Removed remaining `fetch`/`sendBeacon` to `/api/logs/ui-trace` and `/api/logs/client-event` from `overlay.js`.
- Debug overlay — `console` / query `?debug=1` / `?debug-subtitles=1` only.
- **`writeDebug`** — `console.debug` only with `?debug=1` (previously `console.log` wrote every WS frame in production).

### Codebase cleanup — dead code & i18n prune

- **HTTP:** `POST /api/runtime/start` — removed unused `device_id` from request body (client sends only `config_payload`).
- **Browser ASR:** `transport_id` field removed from `IngestedAsrUpdate` (routing by handler parameter, not struct field).
- **TTS:** removed deprecated `enqueue_speech` (Rust), `legacyEnqueue` / `bootstrapTtsTheme` (TS); IPC `tts_enqueue` kept for backward compatibility → use `tts_channel_enqueue`.
- **i18n:** removed obsolete SST keys (legacy local ASR UI, RNNoise tuning UI, removed diagnostics keys) from `scripts/i18n-source/`; catalogs **876** keys × 5 locales.
- **i18n pipeline:** `npm run i18n:bundle` → `scripts/build-locale-bundle.mjs` (regenerate `locales-bundle.js` + copy to `bin/overlay/shared/js/i18n/`).
- **Diagnostics UI:** `rnnoise_message` removed from `diagnostics-normalizer.ts` (API stub in `asr_diagnostics.rs` unchanged).
- **Tests:** dashboard preview contract moved to `SubtitleOutputPreview.svelte` (`tests/renderer/dashboard-panel.contract.test.ts`).

### Toolchain — Rust 2024

- Workspace **edition 2024**, **rust-version 1.85**.
- **`rustfmt.toml`** — unified format; `cargo fmt --all` across workspace.
- Mass alignment of `let` chains / import order in crates (behavior unchanged).

### DevOps — CI and commit conventions

- **`.github/workflows/ci.yml`** — Windows: `rustfmt`, `clippy -D warnings`, `cargo test --workspace`; Ubuntu: `svelte-check`, Vitest, `npm run build`.
- **Husky** + **commitlint** (Conventional Commits); `docs/COMMIT_CONVENTIONS.md`, `.commitlintrc.cjs`.

### Documentation

- **`AGENTS.md`** — rewritten for post-port canon (no SST port / legacy / roadmap).
- Removed: `VOICESUB_ENGINEERING_CONTRACT.ru.md`, `docs/plans/voicesub_roadmap.ru.md`, entire `docs/plans/` folder.
- **`TECHNICAL_ARCHITECTURE`** (en/ru) — updated: no legacy/SST port framing; single canon for agents and development.
- **`README`**, **`WIKI`**, **`TECHNICAL_ARCHITECTURE`** — cleanup pass: i18n pipeline, overlay debug, API/TTS deprecations.

### Contracts (unchanged vs 0.5.2)

| Surface | Transport | Change in 0.5.3 |
| --- | --- | --- |
| OBS overlay | `ws://…/ws/events` | **none** (payload); debug logging gated `?debug=1` |
| Browser worker ASR | `/ws/asr_worker` | **none** (worker page UI/loopback for settings API only) |
| Subtitle/translation lifecycle | Rust core | **none** |
| `config_version` | TOML | **none** |
| `POST /api/runtime/start` | HTTP JSON body | removed unused `device_id` (clients send only `config_payload`) |
| TTS IPC | Tauri | `tts_channel_enqueue` preferred; `tts_enqueue` deprecated compat |

### Breaking / migration notes

- **No `config_version` change.** TOML from 0.5.2 compatible.
- **SST `config.json` import** — `updates.enabled` may become `true` after normalize (if VoiceSub repo and no explicit opt-out).
- **Dashboard navigation** — same settings, new menu structure; command palette updated for `NavTarget`.

### Desktop release format

- **`VoiceSub_0.5.3_x64-setup.exe`** — Tauri 2 NSIS (`installMode: currentUser`).
- Build: `build-release-msi.bat` → `build-release.ps1` → `F:\AI\VoiceSub - release\v0.5.3\`.

### Tests

```powershell
cargo test --workspace
npm run build
npm run test:frontend
```

- **New:** loopback/navigation/shell Vitest suite; `normalize_updates_*`; `authed_api` integration helper; `dashboard_nav.rs` unit tests.

## 0.5.2

Patch release. `PROJECT_VERSION` — **0.5.2**; `config_version` **8** (unchanged). Relative to [v0.5.1](https://github.com/kiriuru/VoiceSub/releases/tag/v0.5.1): TTS speech/twitch hot path moved to Rust; dashboard and TTS UI receive live state via Tauri in-process events instead of localhost WebSocket; OBS Closed Captions send algorithm fixed (clear/timing/dedup); Chrome worker profile/launch stabilization. HTTP/WebSocket contracts for **OBS overlay** (`/overlay`) and **browser worker** (`/ws/asr_worker`) **unchanged**.

### HTTP — loopback API auth + overlay liveness

- **`/api/*`** — per-session `x-voicesub-token` header (CSRF/cross-origin hardening); token in trusted HTML (dashboard, worker, TTS) + Tauri `get_loopback_api_token`.
- **`GET /live`** — public minimal liveness (`{"ok":true}`) for OBS overlay; **`/api/health`** remains protected.
- **OBS overlay** — removed HTTP POST to `/api/logs/*`; debug only `console` / `?debug=1` / `?debug-subtitles=1`; cache-bust `overlay.js?v=20260615a`.
- **Follow-up:** worker/TTS `initLoopbackApiToken` on boot; OAuth callback after init; `ui-config-sync` skips POST without token; injection tests on `/google-asr` and `/tts`; dedup `authed_api` test helper.

### TTS — Rust speech pipeline (hot path)

- **`TtsSpeechPipeline`** (`crates/voicesub-tts/src/speech_pipeline.rs`) — subtitle → plan → Google TTS fetch → enqueue → `PlaybackHub` fully in Rust; TTS WebView **does not participate** in live subtitle and Twitch chat playback.
- **`ChannelOrchestrator`** — prefetch, pump, chunk playback, stuck watchdog, completion waiter on Rust side (speech + twitch channels).
- **`google_fetch.rs`** — HTTP fetch/chunk Google TTS (reqwest pool + keepalive); sidecar `google_tts_fetch.exe` — fallback path.
- **`playback_policy.rs`** — effective playback rate / queue depth for sonic backlog (policy ported from JS).
- **Speech activity UI** — Tauri event `tts-speech-activity` + `src-tts/lib/speech-activity-log.ts` (queue “what is playing” without WS polling).
- **Sample test** in TTS UI remains on `SpeechEngine` (manual preview); live speech/twitch — Rust pipeline only. *(Fixed in **0.5.4**: `tts_speak_sample` → Rust orchestrator.)*

### Runtime — EventBus + snapshot

- **`RuntimeEventBus`** (`crates/voicesub-ws/src/event_bus.rs`) — in-process broadcast parallel to WS publisher; revision counter for reconnect.
- **Dashboard** (`src/lib/runtime-events.ts`) — Tauri channel `runtime-event` + IPC `get_runtime_state_snapshot` instead of `ws://127.0.0.1:8765/ws/events` for live state in main shell.
- **TTS module** (`src-tts/App.svelte`) — same `runtime-event` transport; **`src-tts/lib/ws.ts` removed**.
- **`RuntimeStateSnapshot`** — on connect: runtime, subtitle, overlay, translation, diagnostics (replay without WS handshake race).

### OBS Closed Captions — send algorithm

- **501 (stream not running)** — after successful debug mirror `SetInputSettings`, `clear_after_ms` scheduled (SST parity; Text Source does not stick).
- **Stale clear** — cancel pending `DelayedClear` on new final phrase (source final / superseding payload) **before** sleep worker; no premature wipe between phrases.
- **`payload_will_supersede_caption`** — bump generation only when payload actually replaces caption; dedup and `completed_block_visible: false` **do not cancel** pending clear.
- **Partial native** — after OBS 501, no repeated `SendStreamCaption` on partial (`source_live`); debug mirror continues growing.
- **Unicode partial throttle** — `.chars().count()` instead of byte `len()` (Cyrillic/CJK).
- **Tests:** 32 scenarios in `voicesub-obs` (SST parity + clear race + 501 debug clear + dedup/partial guard).

### Browser worker — launch stability

- **`launch_stability.rs`** — stability profile overrides for Chrome flags (anti-throttle parity SST).
- **`profile_bloat_guard.rs`** — prepare/cleanup `--user-data-dir` worker profile.
- **`process_affinity.rs`** — CPU affinity mask for browser worker on Windows.
- **Chrome launch contract tests** — `crates/voicesub-browser/tests/chrome_launch_contract.rs`.

### ASR worker (Web Speech) — overlap and browser-trace

- **`overlap-logic.ts`** — dual-slot overlap with `continuous=false`: buddy prestart **only after final** (SST parity); `handleInactiveOverlapBuddyEnded` (buddy `onend` without global restart); `shouldIgnoreOverlapBuddyError` for expected buddy `no-speech`/`aborted`/`network`; ghost buddy recovery after sustained idle; handoff resets stale `pendingRestartReason`.
- **Overlap telemetry** — `buildOverlapTelemetrySnapshot` in every worker status/heartbeat (`overlap_mode_desired`, `overlap_active`, slot listening/prestart flags); Rust `GatewayDiagnostics` + overlap fields in `browser_worker_status` / heartbeat; events `browser_overlap_buddy_ended` / `browser_overlap_buddy_error` / `browser_overlap_buddy_ghost_recovered` in `browser-trace.jsonl`.
- **Hotfix overlap prestart** — removed speech/partial and time-based buddy prestart (caused both slots listening simultaneously → `aborted` + ping-pong handoff ~1–2 s); expected handoff `aborted` no longer emits `browser_onerror`.
- **`recognition-handlers.ts`** — aligned overlap guards, `overlapResultAllowed` as in SST.
- **Vitest:** `src-worker/lib/asr/overlap-logic.test.ts` (overlap lifecycle, ghost, telemetry snapshot).

### TTS / Twitch — fixes on top of 0.5.1

- **Queue recovery** — closing TTS WebView mid-playback no longer leaves Rust queues in `Speaking`; force-idle speech+twitch on close/reopen and on module mount (`channel_queue.rs`, `src-tauri/src/tts.rs`).
- **Underscore in Twitch TTS** — `_` in default `strip_symbols`; `replace_underscore_with_space` for spoken nick and message text.
- **GitHub updates slug** — canonical repo **`kiriuru/VoiceSub`** + migration legacy `stream_sub_translator` / `kiriuru/voicesub` in config normalize.

### Dashboard / UI

- **Anime UI theme preset** — localized preset label (`ui-theme-presets.ts`, i18n).

### Contracts (unchanged vs 0.5.1)

| Surface | Transport | Change in 0.5.2 |
| --- | --- | --- |
| OBS overlay | `ws://…/ws/events` | **none** |
| Browser worker | `/ws/asr_worker` | **none** |
| Subtitle/translation HTTP API | Axum | **none** |
| Dashboard (Tauri main webview) | Tauri `runtime-event` | **yes** — instead of localhost WS |
| TTS module webview | Tauri `runtime-event` | **yes** — instead of localhost WS |

### Breaking / migration notes

- **No `config_version` change.** User TOML/JSON from 0.5.1 compatible.
- **Dashboard/TTS live updates** — only inside Tauri shell (`VoiceSub.exe`); external browser on `:8765` can still use WS (OBS path).
- **TTS hot path** — JS `speechEngine`/`twitchEngine` pump for live subtitle/twitch **not used**; UI settings/save IPC semantics unchanged.

### Desktop release format

- **`VoiceSub_0.5.2_x64-setup.exe`** — Tauri 2 NSIS (`installMode: currentUser`).
- Build: `build-release-msi.bat` → `build-release.ps1` → `F:\AI\VoiceSub - release\v0.5.2\`.

### Tests

```powershell
cargo test --workspace
npm run build
npm run test:frontend
```

- **New:** `src/lib/runtime-events.test.ts`, `src-tts/lib/speech-activity-log.test.ts`, `src-worker/lib/asr/overlap-logic.test.ts`, `crates/voicesub-browser/tests/browser_asr_gateway.rs` (overlap telemetry mapping), `chrome_launch_contract.rs`, extended `voicesub-obs` send integration suite, `voicesub-ws` event bus tests.

## 0.5.1

Patch release. `PROJECT_VERSION` in `voicesub-types::version.rs` — **0.5.1**; `config_version` **8** (unchanged). Relative to [v0.5.0](https://github.com/kiriuru/VoiceSub/releases/tag/v0.5.0): native dual-sink TTS (Rust/cpal), Sonic mode instead of browser HTMLAudio, Twitch multi-channel (up to 5 IRC) + hot-apply filters, digit preservation in chat, long-session stabilization (log rotation, WebView2 power/memory, telemetry), smart TTS queue. HTTP/WebSocket subtitle/translation contracts **unchanged**. GitHub release: [v0.5.1](https://github.com/kiriuru/VoiceSub/releases/tag/v0.5.1). Repository: **`kiriuru/VoiceSub`** (migration from `stream_sub_translator` on config load).

### TTS — native dual-sink and playback

- **Two independent channels** — `speech` (subtitles) and `twitch` (chat) with separate `SpeechEngine`, Rust queues and WASAPI devices; parallel playback without `HTMLAudio` / `setSinkId`.
- **Playback modes:** `native` (MP3 → cpal @ 1.0×, minimal latency) and **`sonic`** (libsonic tempo stretch, pitch-preserving rate). Legacy `playback_mode: "browser"` **migrates to `sonic`** on `config.toml` load.
- **`HtmlAudioPlayer` removed** — all playback via IPC `tts_play_audio` + `playback-finished` event.
- **UI:** Native / Sonic selector in module header; “Browse audio output” button (`selectAudioOutput`) removed; rate slider hidden in native (fixed 1.0×); live volume/rate formatting (`playback-format.ts`).
- **Twitch panel:** separate WASAPI device; rate override only in sonic; removed messages about unsupported `setSinkId`.
- **Native device hint banner** — reminder to rebind speech/Twitch devices after native routing transition.
- **Resource telemetry bar** — handle count + private commit for TTS process, `voicesub-app.exe` and `obs64.exe`; refresh 30 s; warning at ≥10k handles or ≥3 GB commit; help popover (en/ru/ja/ko/zh).
- **Progressive Google TTS prefetch** — chunk 0 first, rest parallel; prefetch ahead 2 → 4; HTTP pool (8 idle/host) + TCP keepalive 30 s.
- **Playback rate boost** (`speech-playback-policy.ts`) — speedup when backlog > 2 (up to +0.7); deferred boost for current audible clip.
- **60 s speaking watchdog** — stop player → `mark_finished` → `tts_channel_force_idle` → resume pump.
- **Screen Wake Lock** (`tts-keepalive.ts`) while engines busy; activity IPC `tts_report_webview_activity`.
- **TTS fetch warmup** on module mount; native playback timeout 45 s per chunk.

### TTS — queue and planner (Rust)

- **Adaptive queue drop** on overflow — lowest-priority first (`subtitle_source` < translation < other), up to half capacity; not FIFO drop oldest only.
- **`dedupe_key`** on queue items; dropped/cleared speech items release planner dedupe keys.
- **`ChannelEnqueueResult`** — IPC returns `{ queue_len, dropped_ids }`; JS clears stale prefetches.
- **`force_idle`** — new IPC + service method resets stuck `Speaking` without clearing waiting items.
- **`mark_finished` hardening** — ID mismatch → warn + force idle (does not wedge queue).
- **Device validation** — `tts_set_audio_device` / `tts_set_channel_audio_device` reject unknown device IDs.

### Audio (`voicesub-audio`)

- **Reusable `OutputStream` cache** per channel worker; invalidation on device change or playback error.
- **Sonic streaming** — incremental `SonicProcessor` drain-to-sink (not batch PCM upfront).
- **Poll interval** 50 ms → 10 ms for responsive stop/device-change.
- **At rate 1.0×** — MP3 decode directly without sonic pass.
- **`process_stats.rs` (new)** — Windows telemetry: PID, name, handle count, private commit, working set for `voicesub-app.exe` and `obs64.exe`.

### WebView2 power and memory

- **`webview_power.rs` + `webview2_memory.rs` (new)** — policy: main shell → Normal on focus, LowMemory when unfocused; TTS → Normal when busy/visible, LowMemory when listening+hidden, **Suspend** when fully idle.
- **`webview_memory.rs` (new, Tauri)** — focus/visibility/runtime/tts/busy flags; refresh on focus/visibility events.
- **IPC `tts_report_webview_activity`** — TTS module reports activity for suspend/low-memory.

### Logging

- **`log_rotation.rs` + `rotating_log_file.rs` (new)** — size-based rotation (5 MB, 2 backups) for `core.log`, `runtime-events.log`, JSONL deep traces.
- **Compact client log filter** — by default in `session-latest.jsonl` only overlay/browser_worker/dashboard client logs; TTS-window logs — only with `logging.full_enabled`.
- **TTS UI trace gating** — `tts-trace.ts` does not send `/api/logs/ui-trace` without full logging.
- **Diagnostics ZIP export** — includes deep JSONL traces when `logging.full_enabled`; manifest lists files.

### Subtitle lifecycle

- **Record map pruning** — cap 512 in-memory `records`; drop oldest non-protected sequences (preserves completed, pending, latest final, active partial).

### Dashboard / OBS / overlay

- **`open_local_http_url` Tauri command** — loopback HTTP URLs via system browser (`shell.rs` validation).
- **OBS panel “Open overlay”** — `openLocalUrl()` IPC instead of `window.open` (fallback outside Tauri).
- **Overlay** — removed per-payload `visual_state` UI trace post on every WS update (less overhead in OBS Browser Source).
- **i18n:** `subtitles.overlay_preset.compact` in all 5 dashboard locales + NSIS i18n source.
- **Remote mode cleanup:** removed i18n/UI strings for LAN remote tools, `show_remote_tools` from defaults; SST `remote` section still stripped on import.

### Browser worker / ASR

- **`recognition.abort()`** before clear handlers — clean Web Speech instance release.
- **Mic monitor leak guard** — release stale stream before re-acquire; leak counter.
- **Worker relaunch** — runtime terminate previous Chrome PID before new launch (no orphan processes).
- **Client log throttle map** bounded (max 256 entries).

### IPC / Tauri (new endpoints)

- `tts_channel_force_idle`, `tts_get_resource_telemetry`, `tts_report_webview_activity`, `open_local_http_url`.
- ACL / `build.rs` / autogenerated permissions for new commands.

### Dev / logging defaults

- **`start-voicesub.bat`** — subtitle/UI deep tracing and verbose `RUST_LOG` **disabled by default** (REM); matches release compact logging baseline.
- **New tests:** Vitest (`playback-format`, `resource-telemetry`, `speech-playback-policy`, `tts-keepalive`, `tts-trace`, expanded `google-tts`, `constants`, `obs-status-i18n`, `tts-locale`, `twitch-channels`, `popover-position`); Rust (`voicesub-twitch` 105+ unit: lang/Lingua/links/pipeline/symbols/emoji digits/emotes/service apply_settings, `voicesub-obs` error_codes, audio/sonic, queue adaptive drop, config migration, webview power, log rotation, process stats, lifecycle prune, shell URL validation, session compact filter, `voicesub-browser` launch skip).

### TTS — IPC and Twitch

- **Enqueue IPC** — `ChannelEnqueueResult.dropped_ids` always serialized as `[]` (Rust); normalization and `?? []` in `speech-engine.ts` / `tts-ipc.ts` — fixed TTS failure on empty `dropped_ids`.
- **Twitch language detection** — hybrid Unicode heuristics + **Lingua 1.8** (subset top-20) + whatlang fallback; confidence threshold; removed fragile Dutch word-hints; Ukrainian — heuristics; `TWITCH_TOP_LANGUAGE_CODES` exported from `voicesub-twitch`.
- **Resource telemetry** — `?` button + help popover in TTS module header (`App.svelte`, `tts-module.css`).

### Twitch — multi-channel, chat filters and hot-apply

- **Up to 5 channels on one OAuth connection** — `TwitchTtsSettings.channels` (+ legacy `channel` migration); IRC `JOIN #a,#b,…`; status `channels: Vec<String>` and comma-separated label; “Connection” card with channel list; badge `3/5 channels` / `#shee0n` (`twitch-channels.ts`, Vitest).
- **Symbol filter** — `strip_symbols` field (comma-separated tokens, removed before TTS); `symbols.rs` module; empty list = speak all symbols; default `@, &, $`.
- **Links in chat** — `links.rs`: inline `http(s)://`, `www.`, `watch?v=`, YouTube/Twitch/Discord etc.; `pipeline.rs` — double `strip_links` (before/after `strip_symbols`, so `&` does not break URL); “link-only” / no meaningful text messages → `speakable: false`.
- **Language for link-only / speaker-label** — `strip_leading_speaker_label`, `has_meaningful_linguistic_content`; skip statistical detection for single ASCII tokens; fixed false `[nl]`/`[id]` on lines like `Name: https://youtube.com/…`.
- **Settings apply without IRC reconnect** — `TwitchChatService.apply_settings()` updates `live.chat` on every `tts_update_twitch_settings`; UI: save queue (no parallel `persistSettings` races), checkboxes/numbers — `saveNow()`, text — debounce 400 ms; badge “Saving…” / “Settings applied” (`tts.twitch.settings_*` in 5 tts-locales).
- **“?” help on “Bot nick” field** — explains IRC login of listening account (`tts.twitch.nick_help_*`).
- **TTS light theme** — telemetry metrics (handles / commit) on semantic CSS tokens, readable on light background (`tts-module.css`).
- **Digit preservation in chat** — `strip_unicode_emoji` does not remove ASCII / Arabic-Indic / Fullwidth digits (Unicode `\p{Emoji}` marks `0–9` as emoji components); purely numeric tokens not counted as BTTV/7TV emotes; keycap `5️⃣` → `5`; pipeline + `emoji.rs` + `emotes.rs` tests.
- **“?” hint on “Bot nick”** — `clampPopoverPosition()` (`src-tts/lib/popover-position.ts`): measure popover after mount, shift into viewport (right edge at `?` button), flip up when space lacking; Vitest `popover-position.test.ts`.

### Dev / integration tests — browser worker

- **Chrome does not open in `cargo test`** — `browser_worker_launch_skipped()` (`cfg(test)` + `VOICESUB_SKIP_BROWSER_WORKER`); `cfg(test)` **does not** propagate to integration test dependencies → `integration_lock()` in `voicesub-http/tests/common.rs` and `voicesub-runtime/tests/common.rs` sets `VOICESUB_SKIP_BROWSER_WORKER=1`; stub launch (`pid: 0`) without Chrome spawn; relaunch/stop does not call `taskkill` on nonexistent worker.

### Translation languages and Web Speech (top-20)

- **Translation module** — `LANGUAGES` in `constants.ts`: 20 localization targets (en, zh-cn/zh-tw, ru, es, pt, de, ko, fr, ja, tr, hi, it, ar, pl, id, sv, nl, vi, th); codes in `TRANSLATION_LANGUAGE_CODES`.
- **Web Speech** — `BROWSER_RECOGNITION_LANGUAGES`: top-20 + regional variants (en-US/GB/**AU**, zh-CN/TW, es-ES/MX, …); `uk-UA` kept for existing configs.
- **Vitest** — `constants.test.ts` (list completeness and translation target i18n keys).

### OBS Closed Captions — diagnostics

- **`voicesub-obs/error_codes.rs`** — stable error codes and native-status (`connection_refused`, `stream_active`, …) in API instead of English+OS text in `last_error` / `native_caption_status`.
- **`obs-status-i18n.ts`** — translate codes in UI, parse legacy backend messages (incl. Windows IO 10061 + Russian OS text); `formatObsCcRuntimeStatus`, `formatObsConnectionState`.
- **Panels** — `ObsPanel.svelte`, `ToolsPanel.svelte`, `diagnostics.ts`: localized OBS status without language mixing.

### Localization fix (en / ru / ja / ko / zh)

- Unified i18n pass: **OBS CC** (errors `obs.cc.error.*`, native `obs.cc.native.*`, states `obs.cc.connection_state.*`, Tools string `tools.runtime.obs_cc_status`; fixed `tr()` with `{vars}` substitution in Tools); **translation** (20 target labels `translation.target_lang.*` in `TranslationPanel`); **TTS** (`tts.twitch.max_chars_hint` for ja/ko/zh; `strip_symbols`, `nick_help_*`, `channels_badge`, `settings_saved`/`settings_saving` in 5 tts-locales); **Tools** (`tools.profiles.name_label`); **ko** — placeholder `{error}` instead of `{오류}` in `obs.cc.status.error`. New key source — `scripts/voicesub-locale-overrides.mjs` + `npm run i18n:export`. Tests: `obs-status-i18n.test.ts`, `tts-locale.test.ts`.

### Breaking / migration notes

- **`playback_mode: "browser"`** no longer user-facing; automatically → **`sonic`**.
- **Browser HTMLAudio path removed** — Native or Sonic + WASAPI device selection only.
- **TTS client logs / UI traces** not in `session-latest.jsonl` by default (full logging required).
- **Overlay** no longer emits `visual_state` UI trace on every subtitle update.
- **Twitch `channel` → `channels`** — on TTS `config.toml` load legacy `channel` moved to `channels[0]`; up to 5 logins without `#`; reconnect not required for filter changes (only for channel/OAuth changes).

### Tests

```powershell
cargo test --workspace
npm run build
npm run test:frontend
```

---
## 0.5.0

Major release. Successor to frozen SST `0.4.4`. All items below are **changes relative to SST `0.4.4`**. `PROJECT_VERSION` in `voicesub-types::version.rs` — **0.5.0**; `config_version` **8** (`user-data/config.toml`). Product renamed to **VoiceSub**; HTTP/WebSocket **contracts preserved in meaning** (parity port of subtitle/translation lifecycle), but **stack and delivery fully new**. GitHub release: [v0.5.0](https://github.com/kiriuru/stream_sub_translator/releases/tag/v0.5.0). Formal Phase 1 DoD golden gate — **deferred**.

### Release format (NSIS)

- Artifact: **`VoiceSub_{version}_x64-setup.exe`** (Tauri 2 NSIS, `installMode: currentUser`).
- **`VoiceSub.exe`** — Tauri shell; main webview → `http://127.0.0.1:8765/`.
- Static assets in bundle (`tauri.conf.json` resources): `bin/dashboard/`, `bin/overlay/`, `bin/worker/`, `bin/tts/`, `bin/fonts/`, `bin/modules/`.
- Build: `build-release-msi.bat` → `build-release.ps1` → `npm run build` → TTS sidecar (if needed) → `validate-nsis-i18n.mjs` → `cargo tauri build` (NSIS) → copy `*-setup.exe` to `release_root/v{version}/` from `build/release.config.json`.
- NSIS UI languages: English, Russian, Japanese, Korean, SimpChinese (`src-tauri/windows/installer.nsi`). Legacy WiX `src-tauri/wix/main.wxs` **not used**.
- **Not** in core installer: Python, Node.js, torch, NeMo, pywebview, PyInstaller bootstrap.
- **System dependency:** Google Chrome (or Edge for smoke) for Web Speech worker.
- Splash startup profiles (`Quick Start`, `NVIDIA GPU`, `Remote Controller`, …) **removed** — single entry point.

### Stack and architecture

- **Backend:** Rust Cargo workspace — `voicesub-types`, `voicesub-config`, `voicesub-subtitle`, `voicesub-translation`, `voicesub-browser`, `voicesub-ws`, `voicesub-http`, `voicesub-logging`, `voicesub-export`, `voicesub-obs`, `voicesub-audio`, `voicesub-tts`, `voicesub-twitch`, `voicesub-runtime`; thin `src-tauri/`.
- **HTTP/WS:** embedded Axum on `127.0.0.1:8765` (`VOICESUB_ALLOW_LAN=1` → `0.0.0.0`); routes in `crates/voicesub-runtime/src/http/router.rs`.
- **Dashboard:** Svelte 5 + Vite → `bin/dashboard/` (compile-time; Node.js only on build machine).
- **OBS overlay:** vanilla HTML/JS → `bin/overlay/` (separate from dashboard bundle).
- **Browser Speech worker:** Svelte 5 → `bin/worker/`; pages `/google-asr`, `/google-asr-edge`.
- **Logging:** `tracing` backbone (`logs/core.log`, `logs/runtime-events.log`); opt-in JSONL traces (`VOICESUB_DEEP_DIAGNOSTICS`, `VOICESUB_TRACE_*`).

### Removed from active core (archive `legacy/`)

- Legacy local ASR (`local` mode) — removed from core; SST import maps to `browser_google`.
- Remote controller/worker **removed** (not part of VoiceSub).
- Experimental browser (`/google-asr-experimental*`) → `legacy/experimental-browser/`.
- FastAPI + pywebview + PyInstaller bootstrap SST.
- Splash profiles, `Stream Subtitle Translator Only Web.exe`, `desktop_profile_lock` for unlock legacy local ASR.

### ASR (Browser Speech only)

- Only production core mode: **`browser_google`** (`/google-asr`).
- Chrome supervisor: isolated `--user-data-dir`, visible address bar, anti-throttle flags, EcoQoS opt-out (port of SST `browser_worker_launcher.py`).
- Chrome flags in config: `asr.browser.chrome_launch` (`launch_args`, `disabled_features`); `chrome_flags.rs`, `launch_config.rs`.
- Retry launch without `HIGH_PRIORITY_CLASS` on `ERROR_ACCESS_DENIED` (Windows).
- Worker FSM: `src-worker/lib/asr/session-manager.ts`, `socket-bridge.ts`, force-finalization, session rotation (`max_browser_session_age_ms` default 180000).
- `/api/devices/audio-inputs` — empty list (microphone via Chrome `getUserMedia`).

### Subtitle and translation (Rust port)

- **`voicesub-subtitle`**: `SubtitleLifecycleCore`, `SubtitleRouter`, presentation — SST lifecycle parity (completed block until new final; late translations).
- **`voicesub-translation`**: `TranslationDispatcher` — **13 providers**, slot-aware queue, stale drop, preview supersession `(segment_id, revision)`.
- Golden fixtures: `tests/golden/`, crate-level `golden_*.rs`.

### WebSocket and overlay

- `/ws/events`: replay `runtime_update`, `subtitle_payload_update`, `overlay_update`; stale-guard in dashboard (`src/lib/ws.ts`) and overlay (`bin/overlay/overlay.js`, `ws-stale-guard-logic.js`).
- Overlay reconnect: exponential backoff 1–10 s; last frame preserved on disconnect (OBS UX).
- **Empty overlay cleanup:** `disposeRenderContainer` when `result.empty` (TTL / Stop / idle). **`hasVisibleRenderedFrame()`** in `clearOverlayPresentation` — otherwise idle payload cleared state before `render()` and text remained in OBS. Cache-bust: `overlay.js?v=20260610b`.
- Contract: `crates/voicesub-subtitle/tests/overlay_contract.rs` (`overlay_disposes_renderer_when_payload_is_empty`, `overlay_clears_dom_when_idle_arrives_after_state_already_cleared`).

### TTS module and Twitch

- UI: `src-tts/` → `bin/tts/`, route `/tts`; manifest `bin/modules/tts/module.toml`.
- Rust: `voicesub-tts`, `voicesub-twitch` — queue, subtitle speech planner, IRC, OAuth bridge.
- Tauri IPC: `tts_*` commands (`src-tauri/src/tts.rs`); embedded `google_tts_fetch.exe` in `bin/modules/tts/runtime/` (Python fetcher sources outside git).
- **`TwitchPanel.svelte`:** up to 5 channels, hot-apply filters (`apply_settings`), save queue, nick help popover (`popover-position.ts`); see CHANGELOG §0.5.1.
- **cpal:** enum output devices on separate thread (`list_output_devices_on_thread`).
- API: `/api/tts/google`, `/api/tts/python`, `/api/tts/twitch/oauth-*`.

### OBS Closed Captions

- **`voicesub-obs`**: OBS WebSocket v5 client; config `obs_closed_captions` (SST port semantics).

### Configuration and migrations

- Storage: **`user-data/config.toml`** (JSON-shaped document in TOML).
- `config_version` **8**; SST `config.json` import via `voicesub-config::migrate` (`local` / `remote` / experimental → `browser_google`).
- Profiles: `user-data/profiles/{name}.toml`.
- Env aliases: `VOICESUB_*` + `SST_*` compatibility for deep diagnostics.
- **`normalize_updates_config`:** `[updates]` section supplied on legacy `config.toml` load (upgrade from SST).

### Update check (GitHub Releases)

- **`voicesub-types::version`**: semver compare, `extract_latest_github_release`, `build_version_info_payload`, `release_url_for`.
- **`POST /api/updates/check`** + **`GET /api/version`**: poll GitHub Releases (`updates.github_repo`, channel `stable`/`prerelease`, interval `check_interval_hours`).
- **Startup:** `spawn_startup_check()`; dashboard — check on bootstrap (`UpdateBanner.svelte`).
- **Config defaults:** `updates.enabled: true`, `github_repo: kiriuru/VoiceSub` (legacy `kiriuru/stream_sub_translator` migrates on load).
- **UI:** “new version available” banner (en/ru/ja/ko/zh); **Download** → Tauri `open_external_https_url` (system browser).

### Dashboard UI (Svelte)

- Tabs: Translation, Subtitles, Style, UI Theme, OBS, Word Replace, Tools & Data, Settings, Help.
- Compact layout: Tauri IPC `set_dashboard_layout` (~390×844).
- Command palette, idle subtitle preview (`src/lib/preview-payload.ts`) — placeholder until Start.
- **Translation panel:** duplicate target lang and empty API key validation; gate Save (`translation-helpers.ts`).
- **a11y:** `focus-visible` in `global.css` and command palette.
- i18n: **en, ru, ja, ko, zh** — `src/lib/i18n/locales/*.json` + `tts-*.json`; export `npm run i18n:export`.

### Documentation

- `docs/TECHNICAL_ARCHITECTURE.md`, `docs/TECHNICAL_ARCHITECTURE.en.md` — VoiceSub **0.5.2** (Rust TTS pipeline, RuntimeEventBus, OBS CC send fixes).
- `README.md`, `README.ru.md`, `docs/WIKI.en.md`, `docs/WIKI.ru.md` — updated for new stack.
- **2026-06-10 sync:** MSI → NSIS; overlay TTL cleanup; update check (not stub); update banner; `AGENTS.md`.

### Tests

```powershell
cargo test --workspace
npm run build
npm run test:frontend
```

- Phase 0 automated soak: `voicesub-http/tests/http_ws_smoke.rs::phase0_soak_checklist_automated`.
- `voicesub-config`: `normalizes_updates_defaults_for_legacy_configs`; `voicesub-types`: version/update payload; `overlay_contract.rs`; `translation-helpers.validation.test.ts`; `roundtrip_twitch_ignore_users`.
- Golden parity full suite — **deferred**.

## 0.4.4

> **Frozen line.** SST `0.4.4` — read-only reference (`F:\AI\stream-sub-translator`). Active development — VoiceSub `0.5.5`.

Patch release. `PROJECT_VERSION` in `backend/versioning.py` — **0.4.4**; `config_version` **7**. Public HTTP/WebSocket route contracts and subtitle/translation lifecycle **unchanged**.

Patch release. `PROJECT_VERSION` in `backend/versioning.py` — **0.4.4**; `config_version` **7**. Public HTTP/WebSocket route contracts and subtitle/translation lifecycle **unchanged**.

### Security (OpenAI helper routes)

- **`backend/core/outbound_url_policy.py`**: SSRF policy for `POST /api/openai/models` and `POST /api/openai/usable-models` — on LAN-exposed bind (`0.0.0.0`/`::` or `SST_ALLOW_LAN=1`) loopback, RFC1918, link-local and metadata hostnames forbidden in `base_url`; on default localhost bind private URLs still allowed (local OpenAI-compatible servers). Outbound URL translation providers **unaffected**.
- Regressions: `tests/test_outbound_url_policy.py`, extended `tests/test_openai_models_route.py`.

### Frontend store and desktop bridge

- **`frontend/js/core/store.js`**: `desktop` slice + `patchDesktopContext()` — unified desktop context snapshot for dashboard.
- **`frontend/js/main.js`**: single `sst:desktop-context` listener; `DesktopBridge.getContext()` without duplicate calls.
- **`frontend/js/desktop.js`**: removed dead writes to `window.AppState`.
- Regressions: `tests/test_frontend_architecture.py`, `tests/test_desktop_profile_lock.py`.

### Overlay WebSocket

- **`frontend/js/core/ws-stale-guard-logic.js`**: shared stale algorithm (timestamp-first on sequence reset after stop/start).
- **`frontend/js/core/ws-client.js`**: refactored to shared module.
- **`overlay/overlay.js`**: same stale-filter, exponential reconnect 1–10 s; on disconnect last frame preserved until reconnect (OBS UX).
- Regressions: `tests/test_ws_stale_guard.py`.

### Desktop launcher (module split)

- **`desktop/launcher.py`**: thin facade (re-export); **`desktop/launcher_bootstrap.py`**: `DesktopLauncher`, `main()`, bootstrap/run; mixins **`launcher_window.py`**, **`launcher_backend.py`**, **`browser_worker_launcher.py`**; **`launcher_context.py`**, **`launcher_api.py`**.
- Regressions: `tests/test_launcher.py`, `tests/test_launcher_module_layout.py`.

### Dashboard polish and bind/profile tests

- Bootstrap errors banner in dashboard (`frontend/js/main.js`, `frontend/js/dashboard/actions/data-actions.js`).
- `escapeHtml(label)` in compact nav (`frontend/js/layout/layout-controller.js`).
- **`backend/run.py`**: `resolve_bind_host()` for testable bind policy; **`ProfileManager`**: `resolve()` + `is_relative_to()` for path safety.
- Regressions: `tests/test_bind_policy.py`, `tests/test_profile_manager_paths.py`.

### UI localization (ja / ko / zh)

- **Locales:** dashboard, Browser Speech worker and OBS overlay — **en**, **ru**, **ja**, **ko**, **zh**; selection in header/settings, saved in `ui.language` (`config.json`) and `localStorage` (`sst.ui.language`).
- **i18n architecture change (brief):** instead of two embedded dictionaries in `i18n.js` — separate `frontend/js/i18n/locales/*.js` files, synchronous **`locales-bundle.js`** (all locales in one script for WebView2), **`dynamic-locales.js`** layer for en/ru “late” keys; runtime merge in `i18n.js` (`english ∪ locale ∪ dynamic[locale]`). CJK generation: `tools/generate_i18n_locales.py`, fill `tools/fix_untranslated_cjk.py`, bundle build `tools/build_i18n_locale_bundle.py`.
- **Behavior:** instant switch without fetch; `sst:locale-changed` for panels with dynamic DOM (translation results, ASR, style, overlay); language change immediately writes config via `saveCurrentConfig()`.
- **`desktop/ui_locale.py`**: shared `ui.language` normalization for splash/desktop API.
- Regressions: `tests/test_i18n_locales.py`, `tests/test_i18n_dynamic_locales.py`, `tests/test_ui_locale.py`.
- Details: **§16.8** in `docs/TECHNICAL_ARCHITECTURE.md`.

### Dashboard — ASR advanced (“ASR advanced” tab)

- **`frontend/index.html`**, **`frontend/js/ui/field-help-popover.js`**, **`frontend/js/panels/asr-panel.js`**: each advanced ASR tuning field — `?` button and popup help (popover bottom edge aligned with button; close on repeat click, outside click, `Esc`); mount on `[data-tab-panel="asr_advanced"]`, text update on `sst:locale-changed`.
- **Recommended value labels:** instead of double hint lines like `default:… safer:…` — single line `Recommended: …` / `Рекомендуемое: …` / `推奨:` / `권장:` / `推荐:` (`tools.advanced.*.note`).
- **i18n:** full help texts `tools.advanced.*.help` and `tools.advanced.field_help.aria` for **en**, **ru**, **ja**, **ko**, **zh**; delay preset description uses localized names from `tuning.preset.*` (not slug `balanced` / `ultra low latency` in CJK UI).
- **CSS:** `frontend/css/app.css` — `.field-help-btn`, `.field-help-popover`, `.inline-field-title`; two-column grid `.asr-advanced-fields-grid` (single column on narrow screens); popover/button use theme tokens (`--bg-panel-elevated`, `--line-subtle`), not hardcoded dark fallback.
- **Layout:** side block `tools.notes.*` removed — explanations only via `?` on each field; legacy local ASR extras in grid via `display: contents`; in **compact** — single column (`compact-layout.css`).
- **Maintenance:** `tools/patch_asr_advanced_i18n_cjk.py` (batch note/help update for ja/ko/zh); after locale file edits — `python tools/build_i18n_locale_bundle.py`.
- Regressions: `tests/test_field_help_popover.py`.

### Dashboard preview (idle, before Start)

- **`frontend/js/dashboard/action-helpers.js`**: `buildPreviewPayload` does not replace style-placeholder with empty `overlay_update` from WS after Save while runtime not started — styles can be configured before explicit Start.
- Regressions: `tests/test_dashboard_idle_preview.py`.

### Documentation

- `docs/TECHNICAL_ARCHITECTURE.md`, `docs/TECHNICAL_ARCHITECTURE.en.md` — synced with 0.4.4 (launcher layout, store/overlay WS, pip bootstrap policy, SSRF, **§16.6.1 ASR advanced**, **§16.8 UI i18n**, idle preview §16.7.6).
- `README.md`, `README.ru.md`, `docs/WIKI.en.md`, `docs/WIKI.ru.md` — version and operational notes for 0.4.4 changes.

## 0.4.3

Patch release. `PROJECT_VERSION` in `backend/versioning.py` — **0.4.3**; `config_version` **7**. Public HTTP/WebSocket route contracts and subtitle/translation lifecycle **unchanged**.

### Subtitle renderer (DOM lifecycle, long-session stability)

- **`frontend/js/subtitle-style.js`**: persisted render state (`entrySurfaces`, `partialSurfaceBySlot`, `wrapper`) stores **WeakRef** to DOM nodes (fallback to strong ref without `WeakRef`), so detached surfaces are not retained between frames on slow path (reuse completed source when translation appears).
- Before sole slow-path `container.innerHTML = ""` — `_releaseOrphanedSurfaces`: reset `__sstAppliedStyleMap` on surfaces not in next frame.
- **`disposeRenderContainer(container)`** — explicit state and DOM cleanup; called from **`frontend/js/panels/overlay-panel.js`** on preview unmount and empty payload.
- Regressions: `tests/test_subtitle_style_effects.py` — WeakRef, orphan release, `disposeRenderContainer` contracts.

## 0.4.2

Stabilization release. `PROJECT_VERSION` in `backend/versioning.py` — **0.4.2**; `config_version` **7**. Public HTTP/WebSocket route contracts and subtitle/translation lifecycle **unchanged**.

### Local ASR model integrity (SST idle latency fix)

- **SST local ASR model installer**: added thread-safe cache of local model integrity-check result keyed by `(file_path, mtime_ns, size, expected_sha)`. SHA-256 of multi-GB `.nemo` now computed once per process lifetime instead of every `/api/runtime/status` and `/api/health` call. On fresh install with `sha256` in `manifest.json`, idle-latency status drops from 3–10 s to milliseconds.
- Public API for integrity-cache invalidation; called after manifest write to close race “`shutil.move` → manifest write”.
- Regressions: `tests/test_local_asr_model_installer_manifest.py` — 8 tests, including direct regression `test_integrity_state_caches_sha256_result`.

### Desktop bootstrap install/repair

- **`desktop/bootstrap_payload.py`**: on detected mismatch now cleans existing `app-runtime/` before extracting new payload — drop-in exe replacement updates managed runtime without stale files from previous version.
- **`backend/bootstrap_pip_pins.py`** + `vendor/python-wheels/antlr4_python3_runtime-4.9.3-py3-none-any.whl`: vendored ANTLR4 runtime wheel installed before NeMo to remove flaky sdist build of `antlr4-python3-runtime==4.9.3` on Windows (path/cache/egg-info race in `pip`). Regression: `tests/test_bootstrap_pip_pins.py`.

### Subtitle renderer (incremental effects, no full-line re-render)

- **`frontend/js/subtitle-style.js`** — capability-preserving render flow rewrite:
  - Effect (typewriter / pop-in / glow burst / underline sweep / scale fade / blur sharpen / spotlight pop / ink bloom / vintage flicker) now applied **only to fresh partial fragments**; previously typed portion stays static. CSS classes `.subtitle-fragment-static` / `.subtitle-fragment-fresh` in `frontend/css/subtitle-style.css`.
  - **Shape-signature** for subtitle row (`_shapeSignatureForRows`, `_shapeSignatureForEntry`); if signature matches previous frame, render via fast path (reuses existing wrapper/stage/row/surface DOM) — `container.innerHTML = ""` no longer wipes block on partial source update.
  - Fast path covers transient→completed transition: `_canFastPathFinalize` / `_finalizeTransientSurfaceInPlace` consolidate partial-source into final block without re-animation.
  - Slow path when adding new translation line reuses completed source surface from `previousEntrySurfaces` so source does not re-animate when translation block appears.
  - In `composeRenderRows` partial-source with `lifecycle_state === "completed_with_partial"` marked `transient: true`, so live partial not treated as completed and updates in-place via fast path.
  - New `render_summary` fields: `fast_path_reason`, `finalized_in_place`, `reused_completed_surface`, `reused_partial_surfaces`. Enabled via `SST_TRACE_SUBTITLE_RENDER=1` (see debug channel in `frontend/js/dashboard/ui-trace.js`).
- **`frontend/js/normalizers/overlay-normalizer.js`** + **`overlay/overlay.js`**: `lifecycle_state` now propagated from backend payload to `SubtitleStyleRenderer.render` in both dashboard preview and overlay. Without it fast path misclassified completed_with_partial frames.
- **`frontend/js/panels/overlay-panel.js`**: dashboard preview clears all `.subtitle-stage-note` before adding new one so “Live subtitle block #N” does not multiply frame-by-frame.
- Cache-bust `index.html` / `overlay.html` bump `?v=20260525a` for `subtitle-style.js` / `overlay.js` / `i18n.js` / `main.js` — old cached builds in embedded WebView2 do not pull back old logic.
- Regressions: `tests/test_subtitle_style_effects.py` — ~25 new tests on fast path / shape signature / finalization / lifecycle_state plumbing / DOM reuse.

### Subtitle styles and bundled fonts

- **`backend/core/subtitle_style.py`** — `_STYLE_PRESETS` reworked (10 distinct thematic presets): updated `anime_stream` (Mochiy Pop One + Comfortaa, white fill, narrow purple stroke 1px, soft shadow), `cinema_plate`, `max_contrast`, `comic_burst`, `retro_terminal`; added `fallout_terminal` (Pip-Boy green neon), `cyberpunk_neon`, `noir_caption`, `jp_style` merged into common preset (former “JP dual” removed). `_LEGACY_PRESET_MIGRATIONS` redirects old keys.
- **`fonts/*.ttf`** — 28 popular Google Fonts added directly to repo (Bangers, BebasNeue, Comfortaa, ComicRelief, CutiveMono, Exo2, Inter, JetBrainsMono, Lato, Merriweather, MochiyPopOne, Montserrat, NotoSans, OpenSans, Orbitron, Oswald, PTMono, PlayfairDisplay, Poppins, Raleway, Roboto, ShareTechMono, SourceSans3, SpecialElite, UbuntuMono, Underdog, VT323). All presets use Cyrillic fallback chain (Comfortaa Regular, Lato Regular, Noto Sans, Open Sans) so thematic fonts do not break Russian text.
- **`backend/core/font_catalog.py`**: `_CAMEL_TO_SPACE_RE` normalizes filenames `MochiyPopOne-Regular.ttf` → `Mochiy Pop One Regular` for UI catalog. Regression: `tests/test_font_catalog.py`.
- **Frontend: system fonts not lost on save.** `frontend/js/dashboard/action-helpers.js` exports `mergeFontCatalogPreservingSystem`; `data-actions.js` merges server catalog with client-side `system_font_catalog` cache (localStorage), `config-actions.js` uses same merge on save/import. Previously save overwrote `system` entries on server catalog and user lost selected system font.
- **Style editor UI**:
  - `frontend/js/panels/style/style-editor-panel-shared.js` — `extractPrimaryFontFamily` parses first quoted name from CSS font-family chain so dropdown shows actually selected font on preset load.
  - `frontend/js/panels/style/style-editor-panel-render.js` + `frontend/js/panels/style-editor-panel.js` — new selector `#style-line-slot-apply-preset`: applies base style of selected preset only to specific line slot override, forces `enabled=true` for slot, resets to placeholder after apply.
- **Browser Speech worker (`frontend/google_asr.html`)**: `buildSettingsSavePayload` now loads fresh `/api/settings/load` first, merges only browser-specific fields and saves — previously browser worker window overwrote dashboard changes made between open and “Save”. Regression: `tests/test_browser_worker_contract.py::test_browser_worker_save_reloads_latest_config_before_save`.

### Dashboard UI: compact layout and legacy local ASR settings

- **`frontend/css/compact-layout.css`** — `body.sst-layout-compact .overview-preview-card { display:none !important; }` ensures live snapshot preview not shown in compact view even if DOM moves it outside `.overview-layout`.
- Compact-mode hide rules for decorative `.eyebrow`, `<p class="muted">` under headings and stand-alone `<p class="muted" data-i18n>` now **exclude** technical panels `recognition`, `tuning`, `asr_advanced` via `:not([data-tab-panel="..."])`. Technical hints and notes on legacy local ASR pages remain visible in compact mode (previously aggressively hidden).
- Live snapshot preview card moved in `frontend/index.html` under “Completed text” block (`<pre id="final-transcript">`) so standard and compact layouts group ASR-output blocks equally.
- **`frontend/js/panels/asr/asr-panel-render.js`** — local ASR tuning controls (latency preset, streaming decode toggle, `partial_emit_mode`, `partial_min_new_words`) now visible always except when `desktop_profile_lock="browser_speech"` (Web-Speech-only install). Previously hidden when current `asr.mode === "browser_google"`, user could not configure local ASR before switching mode. Regression: `tests/test_frontend_architecture.py::test_local_asr_tuning_controls_visible_outside_browser_speech_lock`.
- `start-btn` / `stop-btn` marked `type="button"` so they do not trigger nearest form submit and cause stray state reload.

### Opt-in deep-diagnostic tracing

- **`backend/core/diagnostic_flags.py`** (new module) — centralized control via environment variables: `SST_DEEP_DIAGNOSTICS` (master switch) or individual `SST_TRACE_API`, `SST_TRACE_PIPELINE`, `SST_TRACE_UI`, `SST_TRACE_STARTUP_JOURNEY`, `SST_TRACE_RUNTIME_LIFECYCLE`, `SST_TRACE_RUNTIME_EVENTS_VERBOSE`.
- **`backend/core/app_bootstrap.py`** — `configure_api_trace_log`, `configure_ui_trace_log`, `configure_pipeline_trace_log`, `configure_startup_journey_log` called only when flag enabled. JSONL trace files (`logs/api-trace.jsonl`, `logs/pipeline-trace.jsonl`, `logs/ui-trace.jsonl`, `logs/startup-journey.jsonl`) not created and helper functions become no-op without flag.
- **`backend/core/runtime_lifecycle_trace.py`** — `runtime_trace()` short-circuited on `is_runtime_lifecycle_trace_enabled()` (`runtime_lifecycle.*` events in `runtime-events.log` not written when disabled).
- **`backend/core/structured_runtime_logger.py`** — added per-event severity filter. By default only `INF/WRN/ERR/CRT` events written (`translation_publish_accepted`, `browser_external_final`, `browser_degraded`, …). `DBG/VRB` stream (`basr.fsm_transition`, `basr.policy_action_result`, `browser_worker_status`, `translation_queue_depth_changed`, `browser_rearm_scheduled`, …) enabled via `SST_TRACE_RUNTIME_EVENTS_VERBOSE=1` (or master switch). Reduces `logs/runtime-events.log` on normal session ~20–50× (from ~250 KB to ~5–15 KB) matching 0.4.1 disk footprint for install folders.
- **`desktop/launcher.py`** — `configure_startup_journey_log`, `configure_ui_trace_log`, `configure_api_trace_log` now wrapped in `is_startup_journey_enabled()` / `is_ui_trace_enabled()` / `is_api_trace_enabled()` gate so desktop process does not create empty `startup-journey.jsonl` / `ui-trace.jsonl` / `api-trace.jsonl` next to public exe without explicit opt-in (previously created by launcher process independent of backend gate). `deps-install-trace.jsonl` and `subprocess-trace.jsonl` remain always-on for bootstrap triage (small).
- Opt-in deep traces: `SST_TRACE_RUNTIME_EVENTS_VERBOSE` and desktop launcher gates for JSONL traces.
- Regressions: `tests/test_diagnostic_flags.py` (flags off-by-default, master switch, individual flags, truthy tokens, no-op helpers); `tests/test_structured_runtime_logger.py::test_default_skips_dbg_and_vrb_events` (DBG/VRB filter); `tests/test_api_and_websockets.py::test_runtime_start_emits_structured_lifecycle_trace` wrapped `mock.patch.dict("os.environ", {"SST_TRACE_RUNTIME_LIFECYCLE": "1"})`.

### Documentation

Engineering hardening previously in `Unreleased` above `0.4.1` recorded as part of `0.4.2` line:

### Runtime orchestrator and legacy local ASR

- **`RuntimeOrchestrator`** (`backend/core/runtime_orchestrator.py`, ~380 lines) — thin facade with mixin modules: `runtime_orchestrator_{lifecycle,local_asr,browser_worker,diagnostics,state_metrics,remote_ingress}_mixin.py`.
- Extracted local ASR modules: `local_asr_pipeline`, `local_asr_realtime_settings`, `local_asr_recognition_processing`, `local_asr_hallucination_filter`, `local_asr_vad_tuning`, `local_asr_transcript_segment`, `segment_audio_enqueue`, `partial_emit_coordinator`, `realtime_transcript_emit_policy`, `asr_diagnostics_assembler`, `browser_worker_transcript_builders`.
- `PartialEmitCoordinator`: fixed mark → duplicate check order for partial emit.
- `prepare_recognition_audio_bytes`: legacy `experimental_noise_reduction_enabled` + type guard (aligned with `apply_recognition_processing_settings`).
- `local_asr_provider`: `nvidia-smi` result cache (do not block event loop on every diagnostics/status).

### WebSocket and Browser ASR transport

- **`WebSocketManager`**: per-connection `asyncio.Lock` for `send_json` serialization (`_send_json_locked`); `send_direct` for bootstrap `hello` on `/ws/events`; regressions in `test_ws_manager.py`.
- **`BrowserAsrService`**: `_send_lock` for worker socket; `send_hello`; idempotent `disconnect`; reject stale session rollback by `generation_id`; regressions in `test_browser_asr_service.py`.

### Dashboard UI and config (frontend)

- **`store.js`**: `emit()` — snapshot listeners + `try/catch` per listener (one panel failure does not stop others).
- **`dom.js`**: `setInputValueIfChanged`, `setCheckedIfChanged` — idempotent render without caret/focus reset.
- Panels: diagnostics `configJson`, ASR/OBS/overlay/translation/profiles/remote — idempotent DOM updates; ASR/OBS — single `change` handler on checkbox/select; remote panel — no double config subscription; translation panel — import `getLineMap`.
- **`config-normalizer.js`**: `asr.browser.continuous_results` default **true** (`!== false`), aligned with backend.

### Desktop launcher and bootstrap

- **`desktop/launcher.py`**: rotate `desktop-launcher.log` → `desktop-launcher.old.log` (and sibling live logs) on start.
- **`desktop/bootstrap_launcher.py`**: rotate `bootstrap-launcher.log`; GitHub update check timeout **2.5 s**.
- New tests: `test_desktop_launcher_config.py`, `test_desktop_bootstrap_payload.py`, `test_desktop_runtime_bootstrap.py`.

### Documentation

- `docs/TECHNICAL_ARCHITECTURE.md` — full update §6, §9, §14, §16–§17, §20–§21.
- `README.md` / `README.ru.md` — architecture summary, recognition, desktop logs, dashboard UX stability.

### Tests

- `python -m unittest discover -s tests -p "test_*.py"` — **462** collected, **461** OK (1 pre-existing loader error: `test_browser_asr_observability` import `tests.test_translation_dispatcher`).

---
## 0.4.1

Release on top of `0.4.0`. `PROJECT_VERSION = "0.4.1"`; `config_version` remains **7**. Public HTTP/WebSocket contracts and subtitle lifecycle preserved. Delta: [docs/DESKTOP_RELEASE_CHANGELOG_0.4.1.md](./DESKTOP_RELEASE_CHANGELOG_0.4.1.md).

### What’s included

- **Legacy local ASR realtime:** streaming decode, `word_growth` partial policy, delta ASR queue, overlay partial dedup bypass; `LocalAsrPipeline` and `local_asr_realtime_settings`.
- **Dashboard:** latency presets on Tuning, sliders aligned with presets, Save + Stop/Start hints; runtime row with saved realtime profile; Tools → `streaming_decode`, `partial_emit_mode`, `partial_min_new_words` fields, preset mirror.
- **Product:** only low-latency local model preset in UI; migration of old quality preset to low latency.
- **Diagnostics:** extended `AsrDiagnostics` (preset / streaming / emit mode / min words).
- **Documentation:** updated `docs/TECHNICAL_ARCHITECTURE.md` (local ASR pipeline 0.4.1).

### Tests

- `python -m unittest discover -s tests` — run after changes (tracked suite; desktop-only tests — locally when `desktop/` present).

## 0.4.0

Release on top of `0.3.2`. `PROJECT_VERSION = "0.4.0"`; `config_version` remains **7**. Public HTTP/WebSocket contracts and subtitle lifecycle preserved.

### What’s included

- **Browser ASR observability:** `timekeeping.py`, `browser_asr_*` (trace, normalized ingest, operational FSM, recovery policy, JSONL replay); L2 ingress (stale transport / overlap); trace fields on `TranscriptSegment`.
- **WebSocket:** bounded per-connection queues, drop-oldest; `replay_last` (§9 `TECHNICAL_ARCHITECTURE.md`).
- **Translation:** preview supersession in `translation_dispatcher.py`.
- **Compact dashboard:** `ui.layout` `standard` | `compact`; `compact-layout.css`, `layout/layout-controller.js`; desktop-shell window resize on layout change (~1440×940 vs ~400×844).
- **Desktop exe:** second bootstrap `Stream Subtitle Translator Only Web.exe` (Web Speech without splash profiles); in standard exe — 0.4.0 payload with same splash profiles as before.
- **Web Speech quick start:** `asr.desktop_profile_lock` in config schema and after save/load; Recognition without legacy local ASR until GPU/CPU launch (`desktop-profile-lock.js`, normalizers, packaged launcher).
- **Desktop dashboard:** panels mount immediately; help (`dashboard-help-topics.html`) and `loadInitialData()` — in background; `desktop.js` / `main.js` do not block UI on `pywebviewready`.
- **Desktop launcher (pywebview):** transition to dashboard via `location.replace` instead of `load_url`; no `evaluate_js` in splash after navigation; early `GET /` + health in background; do not call `get_current_url()` from `loaded` handler; WebView2 profile in `runtime_root/pywebview-profile`.
- **Fix:** `RuntimeOrchestrator.browser_asr_worker_connected()` — worker WS no longer drops immediately after connect.
- **Tests (GitHub):** `test_browser_asr_observability.py`, `test_frontend_modular_vanilla.py`, extended ws/translation/browser contracts. Desktop packaging tests — local only (`test_desktop_launcher_startup.py`, `test_launcher.py`, …).

### Tests

- `python -m unittest discover -s tests` — **336** tests, `OK` (locally with desktop-only tests; in public repo — tracked suite without `desktop/`).

## 0.3.2

Release on top of `0.3.1`. `PROJECT_VERSION = "0.3.2"`; `config_version = 7` (`source_text_replacement`).

### Version and configuration

- `backend/versioning.py`: `PROJECT_VERSION = "0.3.2"` (source of truth for `GET /api/version` and update check).
- `backend/schemas/config_schema.py`: `CURRENT_CONFIG_VERSION = 7`.
- New config section `source_text_replacement`: optional post-ASR word/phrase replacement before translation, subtitles and OBS captions; does not affect recognition.
- `backend/data/source_text_builtin_pairs.json`: starter pair list (English + Russian), replaceable/extendable with user pairs in UI.
- `backend/core/source_text_replacement.py`, `backend/config/normalizers/source_text_replacement.py`, changes to `LocalConfigManager`, `TranscriptController`, `runtime_orchestrator`.
- `backend/data/config.example.json`: version `7` and `source_text_replacement` block.
- Regressions: `tests/test_source_text_replacement.py`, extended `tests/test_config_migrations.py`, `tests/test_runtime_status_contract.py`.

### Dashboard and i18n

- `frontend/index.html`, `frontend/js/panels/source-text-replacement-panel.js`, `frontend/js/main.js`, `frontend/js/normalizers/config-normalizer.js`, `frontend/js/i18n.js`, `frontend/css/app.css`: **Tools & Data** tab — “After recognition / word replacement” block (on/off, built-in list, case, whole words; custom pairs: two “word” and “replacement” fields, “Add” button, list with checkbox selection and single “Delete selected” button; global **Save** to apply to running backend).
- Help: clarified `help.tools.body` text (EN/RU) about post-ASR layer.

### Web Speech worker (browser)

- `frontend/js/browser-web-speech-recognition-policy.js`: overlap session policy (default with `continuous=false`) and utilities for future on-device/phrase hints.
- `frontend/js/browser-asr-session-manager.js`: dual `SpeechRecognition` instances with buddy prestart after final (reduces gap between Chrome sessions); soft retry on `phrases-not-supported` and one attempt after `language-not-supported` with Chrome on-device hints reset; ignore noisy `aborted` on active slot when buddy already running.

### Subtitles and OBS

- `backend/core/subtitle_style.py`: presets `accessibility_high_contrast`, `dark_cinema`, `meeting_soft` (guides: accessibility, dark scene, calm “meeting” look).
- `backend/core/obs_caption_output.py`, style/versioning changes as release aligned (see branch git history).

### Documentation

- `docs/TECHNICAL_ARCHITECTURE.md`: updated for `0.3.2`, `config_version` 7, `source_text_replacement` flow, **extended legacy local ASR section** (VAD, segment queue, RNNoise, two providers quality vs low-latency, link to `subtitle_lifecycle`).
- `README.md` / `README.ru.md`: version `0.3.2`.

### Tests

- Full run: `python -m unittest discover -s tests` — **298** tests, `OK` (at release freeze).

## 0.3.1

Stabilization release on top of `0.3.0`. `PROJECT_VERSION = "0.3.1"`. Public `/api`/WebSocket unchanged.

### Version and identification

- `backend/versioning.py`: `PROJECT_VERSION = "0.3.1"`, source of truth for `GET /api/version` and `POST /api/updates/check`.
- Bootstrap launcher and desktop shell use same version.

### Bootstrap launcher

- In full dev tree: `desktop/bootstrap_launcher.py`, `desktop/bootstrap_payload.py` (in public GitHub clone `desktop/` may be absent).
- Update check in bootstrap ignores `v2.x` tags when embedded version is `0.x`: old `v2.8.x` releases no longer shown as “newer than `0.3.x`”. Regression — `tests/test_bootstrap_release_tag_filter.py`.

### Web Speech: additional Windows Chrome worker window protection

On top of profile isolation and separate Google Chrome window launch already in `0.3.0`:

- worker Chrome window launched with `HIGH_PRIORITY_CLASS`;
- on Windows 10/11 worker process opts out of EcoQoS / Efficiency Mode via `SetProcessInformation` + `ProcessPowerThrottling`;
- Chrome feature gates `CalculateNativeWinOcclusion`, `HighEfficiencyModeAvailable`, `HeuristicMemorySaver`, `IntensiveWakeUpThrottling`, `GlobalMediaControls` disabled so Web Speech does not “sleep” when window occluded.

### Web Speech: recognition protection inside worker

`frontend/js/browser-asr-session-manager.js`:

- `navigator.wakeLock.request("screen")` while recognition active and window visible; lock automatically re-requested after visibility flip and released on Stop;
- network preflight: after three `network` errors in ~12 s worker tries `https://www.google.com/generate_204`; on failure supervisor enters terminal `recognition_network_unreachable` instead of infinite restart loop;
- health signal `voice_below_recognition_threshold` (RMS ≥ 0.025, accumulated `no-speech`, recognition silence ≥ 8 s);
- early controlled session rotation: `asr.browser.max_browser_session_age_ms` default `180000` ms (was `240000`), window `prepare_cycle_before_ms` remains `15000` ms.

### Translation cache and translation queue

- `backend/core/cache_manager.py` rewritten to in-memory LRU with debounced disk persist (was blocking write on every move from asyncio path), corrupted cache file quarantine preserved.
- `TranslationDispatcher` became restartable (`stop()` no longer “breaks” dispatcher for next sessions), added per-provider concurrency limit and basic rate limiting, per-target-language parallelism preserved.

### Logs

- `backend/core/structured_log_compact.py` — new helper for compressing structured runtime logs (truncate long strings, summarize long lists, depth limit), wired in `structured_runtime_logger`.

### UX and styles

- Built-in subtitle appearance effects added `slide_up`, `zoom_in`, `blur_in`, `glow` (alongside existing `none`, `fade`, `subtle_pop`).
- Minor frontend panel fixes: translation panel and slot cards neater in edge cases, extended i18n strings, targeted ASR/runtime/style panel improvements.

### Documentation

- `docs/CHANGELOG.md` and `docs/TECHNICAL_ARCHITECTURE.md` aligned to unified Russian wording.
- Per-version installer delta for `0.3.1` (formerly separate file; content in this CHANGELOG).

### Already in 0.3.0, not new in 0.3.1

- `RuntimeOrchestrator` decomposition into controllers under `backend/core/runtime/` (state/metrics/session/segment/lifecycle/browser-worker/speech sources/audio capture/processing tasks/translation runtime/transcript/output fanout).
- `SubtitleRouter` split into `subtitle_lifecycle_core.py` + `subtitle_presentation.py` + facade.
- Package `backend/translation/` (`base.py`, `engine.py`, `readiness.py`, `registry.py`) and provider package `providers/*`.
- `backend/core/atomic_io.py` and atomic config/profiles write.
- `backend/services/config_state_service.py` (`ConfigStateService` with explicit lock and active config snapshot metadata).
- `backend/services/update_service.py` + `POST /api/updates/check` + `runtime_start_snapshot` protection from update metadata writes.
- OpenAI helper endpoints `GET /api/openai/recommended-models`, `POST /api/openai/models`, `POST /api/openai/usable-models`.
- Cards `translation_1..translation_5`, `TranslationLineConfig`, migration `subtitle_output.display_order` to translation slot ids.
- Web Speech worker in Google Chrome separate window with address bar and isolated `--user-data-dir`, `asr.browser.worker_launch_browser` values `auto`/`google_chrome`.
- Web Speech supervisor (`browser-asr-session-manager.js`), experimental `/google-asr-experimental`, UI theme/palette, Help tab, extended i18n, runtime-event coalescing and `/ws/events` / `/ws/asr_worker` stability.
- `GET /api/exports/diagnostics` (ZIP with runtime/config/log/session data) and best-effort `/api/logs/client-event`.

### Tests and verification

- `python -m compileall backend desktop tests`
- `.\.venv\Scripts\python.exe -m unittest discover -s tests -p "test_*.py"`

Result:

- `283 tests`
- `OK`

## 0.3.0

Architectural release moving backend to explicit services/schemas/bootstrap layers, modular frontend without build step, config migrations and schema export, new runtime/browser ASR resilience layer and documented experimental browser worker path.

### Main changes

- backend split into `api/routes`, `services`, `core`, `schemas` without changing base local-first product;
- `app.state` no longer assembled manually in one `app.py`, raised via centralized bootstrap;
- config gained explicit `config_version` migrations and JSON Schema export;
- dashboard moved from monolithic `app.js` to ES modules with `core/`, `dashboard/`, `panels/`, `normalizers/`;
- Browser Speech lifecycle extracted to separate supervisor/session manager, more resilient to `onend`, `no-speech`, reconnect and stale worker state;
- `/ws/events` and `/ws/asr_worker` got safer reconnect, dead socket and stale browser worker generation handling;
- client-event logging in best-effort mode no longer should crash backend on live event log write errors;
- overlay/runtime path better survives duplicate/stale event storms and late translation updates;
- separate experimental page `/google-asr-experimental` included in release as supported experimental path based on `SpeechRecognition.start(audioTrack)`;
- local AI path and `browser_google` not removed; legacy local ASR remains available;
- unsupported backend ASR experiments removed from active product surface; only legacy local ASR and browser worker modes remain.

### Backend architecture

- added and wired `backend/services/runtime_service.py`, `settings_service.py`, `asr_service.py`, `translation_service.py`, `diagnostics_service.py`, `export_service.py`, `overlay_service.py`, `model_manager_service.py`;
- introduced `backend/core/app_bootstrap.py` as single init point for runtime paths, managers, services and orchestrator wiring;
- extracted shared utilities:
  - `backend/core/paths.py`
  - `backend/core/logging_setup.py`
  - `backend/core/api_errors.py`
  - `backend/core/redaction.py`
- `backend/runtime_paths.py` kept as compatibility shim over new paths layer;
- routes thinner, delegate orchestration to app services;
- `backend/api/routes_profiles.py` moved to more structured API error payload.

### Configuration, migrations, schema

- config moved to explicit migrations via `backend/core/config_migrations.py`;
- profiles and main config share common migration/normalization pipeline;
- added schema export via `backend/core/config_schema_export.py`;
- schema published in `backend/data/config.schema.json`;
- extended Pydantic schema modules in `backend/schemas/` for config/runtime/asr/translation/overlay/diagnostics;
- migration v3 moves legacy realtime preset to low-latency preset;
- obsolete historical backend ASR settings on normalize return to supported local ASR defaults.

### Frontend modularity

- dashboard entry — `frontend/js/main.js`;
- new module stack:
  - `frontend/js/core/`
  - `frontend/js/dashboard/`
  - `frontend/js/panels/`
  - `frontend/js/normalizers/`
- store/API/WebSocket/events/logging extracted to separate modules;
- panel logic split by domain instead of one growing file;
- normalizers — separate pure functions, test-friendly;
- stack unchanged in principle:
  - plain HTML/CSS/JS
  - served via FastAPI static
  - no Node.js, React, Vite, Webpack or any build pipeline.

### Browser Speech resilience

- browser recognition lifecycle extracted to `frontend/js/browser-asr-session-manager.js`;
- supervisor with states: `idle`, `starting`, `running`, `stopping`, `restarting`, `backoff`, `fatal`;
- removed old chaotic `start/stop/onend` loop;
- `recognition.start()` no longer called on top of `stopping`, deferred to controlled restart;
- cooldowns by reason: `normal_onend`, `settings_change`, `websocket_reconnect`, `watchdog_stall`, `no_speech`, `network`;
- worker diagnostics (generation/session id, FSM state, duplicate/network error counters, mic health);
- browser worker reconnects should not leave runtime in stale `listening/stopping`;
- experimental `/google-asr-experimental` synced with same base FSM.

### WebSocket and runtime event resilience

- `backend/ws_manager.py` safer under contention, more tolerant of disconnect/send errors;
- dead sockets removed after `WebSocketDisconnect`, `RuntimeError`, `OSError`, `ConnectionResetError`, `BrokenPipeError`;
- runtime/browser worker events handled with sequence and staleness awareness;
- avalanche of duplicate `runtime_status -> listening` suppressed by coalescing logic;
- `/ws/events` reconnect should not spawn active client loops and old timers;
- Windows-level close errors `WinError 10022` handled as disconnect cleanup.

### Logging and diagnostics

- `/api/logs/client-event` moved to best-effort mode;
- `SessionLogger` creates log directory upfront, does not hold problematic file handle permanently and counts dropped events;
- structured runtime logs strengthened, sensitive fields redacted.

### ASR surface cleanup

- current ASR surface limited to legacy local ASR and two browser worker modes;
- removed/unsupported backend ASR experiments cleaned on migration and config save/load.

### Tests

Control check on final `0.3.0` change set:

- `python -m compileall backend tests`
- `.\.venv\Scripts\python.exe -m unittest discover -s tests -p "test_*.py"`

Result: `135 tests`, `OK`.

## 0.2.9.x

`0.2.9.*` history remains in archived release notes and is not maintained in this main changelog file.
