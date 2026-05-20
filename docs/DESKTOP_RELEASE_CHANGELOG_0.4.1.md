# SST Desktop 0.4.1

## Stream Subtitle Translator 0.4.1

**Scope:** application payload `PROJECT_VERSION = "0.4.1"` (see `backend/versioning.py`). `config_version` remains **7**. Public HTTP/WebSocket contracts and subtitle/translation lifecycle semantics are unchanged.

### Highlights

- **Local Parakeet realtime (Web Speech–style):** incremental NeMo streaming decode (`streaming_decode`), `word_growth` partial emission (`RealtimeTranscriptEmitPolicy`), delta ASR queue (`segment_audio_enqueue`, `audio_is_delta`), overlay broadcaster dedup bypass for partial-heavy payloads.
- **Runtime extraction:** `LocalAsrPipeline`, `local_asr_realtime_settings`, `local_asr_constants` — thinner `RuntimeOrchestrator` facade.
- **Dashboard UX:** Tuning tab latency presets (`ultra_low_latency` / `balanced` / `quality` / `custom`) aligned with backend preset tables; Quick Tuning sliders snap to fixed positions for named presets; save/restart guidance; Runtime strip shows saved Parakeet realtime summary + engine `true_streaming` when diagnostics are available.
- **Tools → ASR advanced:** latency preset mirror, `streaming_decode`, `partial_emit_mode`, `partial_min_new_words` bound to config (same mutators as Overview).
- **Product simplification:** local ASR is **Official EU Parakeet Low Latency** only — legacy non–low-latency provider UI and preference are removed; configs migrate to `official_eu_parakeet_low_latency`.
- **ASR diagnostics:** `AsrDiagnostics` echoes `active_latency_preset`, `streaming_decode`, `partial_emit_mode`, `partial_min_new_words` for the dashboard.

### Verification

- Full `unittest` suite (tracked + local desktop modules as applicable).
- Manual: local mode — change Tuning preset → Save → Stop/Start — partials grow word-by-word from phrase start; translation lifecycle unchanged (old translation until new **final**).

### Локализация (RU)

- Те же пункты: realtime Parakeet, пресеты и слайдеры, подсказки Save/Stop/Start, строка статуса runtime, поля Tools, единственный провайдер low latency, диагностика.

### Build layout note

- After `publish-desktop-releases*.ps1`, expect `dist/desktop-releases/v0.4.1/` (`01-bootstrap-onefile/`, `01-bootstrap-web-only-onefile/`, `02-managed-app-onefolder/`, `03-installers-both/`, `README.txt`).

### Engineering follow-up (same `0.4.1` code line, not a separate product release)

Documented in [docs/CHANGELOG.md](./CHANGELOG.md#unreleased) and [docs/TECHNICAL_ARCHITECTURE.md](./TECHNICAL_ARCHITECTURE.md):

- thin `RuntimeOrchestrator` facade + mixins; expanded local ASR module tests;
- WebSocket `/ws/events` send mutex + `BrowserAsrService` worker send lock;
- dashboard store/panel UX hardening; desktop log rotation (`*.old.log`);
- **462** collected unit tests (**461** OK; one pre-existing `test_browser_asr_observability` import issue).
