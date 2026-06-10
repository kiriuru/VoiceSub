@echo off
setlocal
cd /d "%~dp0"

where python >nul 2>&1
if errorlevel 1 (
  echo Python 3 is required on the build machine to compile the embedded TTS fetcher.
  exit /b 1
)

python -m pip install --upgrade nuitka >nul 2>&1
python "%~dp0build_runtime.py"
exit /b %ERRORLEVEL%
