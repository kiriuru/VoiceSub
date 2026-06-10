# Back-compat entry point — MSI replaced by single NSIS setup.exe (see build-release.ps1).
& (Join-Path $PSScriptRoot "build-release.ps1")
exit $LASTEXITCODE
