$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)

& npm.cmd ci --no-audit --no-fund
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& (Join-Path $PSScriptRoot 'bundle-python.ps1') -Platform win-x64
Write-Host 'Dipendenze pronte. Esegui npm.cmd run desktop:build.'
