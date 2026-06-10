@echo off

setlocal EnableExtensions

cd /d "%~dp0"



echo VoiceSub NSIS release build (single setup.exe)

echo.



where powershell >nul 2>nul

if errorlevel 1 (

  echo [error] powershell not found

  pause

  exit /b 1

)



powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-release-msi.ps1"

set "EXIT_CODE=%ERRORLEVEL%"



echo.

if not "%EXIT_CODE%"=="0" (

  echo Release build failed with code %EXIT_CODE%.

) else (

  echo Release build finished.

)

pause

exit /b %EXIT_CODE%

