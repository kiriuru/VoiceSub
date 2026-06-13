@echo off
setlocal EnableExtensions
cd /d "%~dp0"

set "SKIP_BUILD=0"
if /i "%~1"=="--no-build" set "SKIP_BUILD=1"
if /i "%~1"=="/no-build" set "SKIP_BUILD=1"

echo [VoiceSub 0.5] Dev launcher (Rust + Tauri)
echo Project: %CD%
echo.

where cargo >nul 2>nul
if errorlevel 1 (
  echo [error] cargo not found. Install Rust: https://rustup.rs/
  pause
  exit /b 1
)

where npm >nul 2>nul
if errorlevel 1 (
  echo [error] npm not found. Install Node.js for frontend build only ^(not used at runtime^).
  pause
  exit /b 1
)

if "%SKIP_BUILD%"=="0" (
  echo [1/2] Building dashboard and worker static assets...
  call npm run build
  if errorlevel 1 (
    echo [error] npm run build failed
    pause
    exit /b 1
  )
) else (
  echo [1/2] Skipping frontend build ^(--no-build^).
)

REM Optional dev tracing (off by default — matches release compact logging):
REM set "VOICESUB_TRACE_SUBTITLE=1"
REM set "RUST_LOG=voicesub_subtitle=debug,voicesub_runtime=debug"
echo.

echo [maintenance] Pruning stale target/incremental caches if over 5 GiB...
cargo run -q -p xtask -- prune-target --if-needed 5 >nul 2>nul

echo [2/2] Starting VoiceSub desktop shell...
echo   Dashboard: http://127.0.0.1:8765/
echo   Overlay:   http://127.0.0.1:8765/overlay
echo   Worker:    http://127.0.0.1:8765/google-asr
echo.
echo Tip: faster restart after Rust-only edits: start-voicesub.bat --no-build
echo.

cargo run -p voicesub-app
set "EXIT_CODE=%ERRORLEVEL%"

echo.
if not "%EXIT_CODE%"=="0" (
  echo VoiceSub exited with code %EXIT_CODE%.
) else (
  echo VoiceSub stopped.
)
pause
exit /b %EXIT_CODE%
