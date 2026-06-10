# VoiceSub release build — single NSIS setup.exe to build/release.config.json -> release_root
$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ConfigPath = Join-Path $ProjectRoot "build\release.config.json"

if (-not (Test-Path $ConfigPath)) {
    throw "Missing release config: $ConfigPath"
}

$releaseConfig = Get-Content $ConfigPath -Raw | ConvertFrom-Json
$ReleaseRoot = [string]$releaseConfig.release_root
if ([string]::IsNullOrWhiteSpace($ReleaseRoot)) {
    throw "release_root is empty in $ConfigPath"
}

Set-Location $ProjectRoot

function Require-Command($Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command not found: $Name"
    }
}

Require-Command "npm"
Require-Command "cargo"

$tauriConfigPath = Join-Path $ProjectRoot "src-tauri\tauri.conf.json"
$tauriConfig = Get-Content $tauriConfigPath -Raw | ConvertFrom-Json
$Version = [string]$tauriConfig.version
$ProductName = [string]$tauriConfig.productName

Write-Host "VoiceSub NSIS release build v$Version"
Write-Host "Project:  $ProjectRoot"
Write-Host "Publish:  $ReleaseRoot"
Write-Host ""

Write-Host "[1/4] Frontend static assets..."
npm run build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
npm run build:tts
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[2/4] TTS embedded runtime..."
$embeddedTts = Join-Path $ProjectRoot "bin\modules\tts\runtime\win-x64\google_tts_fetch.exe"
if (-not (Test-Path $embeddedTts)) {
    Write-Host "Building embedded TTS fetcher (Nuitka)..."
    $ttsBuild = Join-Path $ProjectRoot "bin\modules\tts\build_runtime.bat"
    if (-not (Test-Path $ttsBuild)) {
        throw "TTS build script missing: $ttsBuild"
    }
    & cmd /c $ttsBuild
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
if (-not (Test-Path $embeddedTts)) {
    throw "Embedded TTS fetcher still missing after build: $embeddedTts"
}
Write-Host "TTS runtime: $embeddedTts"

Write-Host "[3/4] NSIS i18n audit + Tauri release build (NSIS setup.exe)..."
node scripts/validate-nsis-i18n.mjs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
# Force a predictable target dir so publish never picks a stale sandbox/cached setup.exe.
$env:CARGO_TARGET_DIR = Join-Path $ProjectRoot "target"
cargo tauri build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[4/4] Publishing setup.exe..."
$searchRoots = @(
    $ProjectRoot,
    (Join-Path $ProjectRoot "target"),
    (Join-Path $ProjectRoot "src-tauri\target")
)
if ($env:CARGO_TARGET_DIR) {
    $searchRoots = @($env:CARGO_TARGET_DIR) + $searchRoots
}

$setup = $null
foreach ($root in $searchRoots) {
    if (-not (Test-Path $root)) { continue }
    $found = Get-ChildItem -Path $root -Recurse -Filter "*-setup.exe" -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match "\\bundle\\nsis\\" } |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if ($found) {
        $setup = $found
        break
    }
}

if (-not $setup) {
    throw "NSIS setup.exe not found. Run cargo tauri build and check target\release\bundle\nsis\"
}

$destDir = Join-Path $ReleaseRoot ("v" + $Version)
if (Test-Path $destDir) {
    Get-ChildItem -LiteralPath $destDir -Force | Remove-Item -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $destDir | Out-Null

$destFile = Join-Path $destDir $setup.Name
Copy-Item -Path $setup.FullName -Destination $destFile -Force

Write-Host ""
Write-Host "OK: $destFile"
Write-Host "Publish folder contains the installer exe only."
