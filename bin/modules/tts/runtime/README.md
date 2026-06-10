# TTS module embedded Python runtime

Shipped layout (built locally or in CI, not required in git for dev):

```
bin/modules/tts/runtime/
  win-x64/google_tts_fetch.exe
  macos-arm64/google_tts_fetch
  macos-x64/google_tts_fetch
  linux-x64/google_tts_fetch
```

Build (developer machine only):

```bat
bin\modules\tts\build_runtime.bat
```

VoiceSub uses this embedded binary instead of system `python`/`py`.
`build_runtime.py` tries Nuitka first, then PyInstaller if Nuitka fails (common on embeddable CPython).
Dev debug builds may fall back to `google_tts_fetch.py` + system Python when the embedded binary is missing.

**Rebuild after changing `google_tts_fetch.py`** (Cyrillic/UTF-8 fixes require a fresh binary).

Reference: [twitchTransFreeNext](https://github.com/sayonari/twitchTransFreeNext) bundles Python with Nuitka (`build.py`).
