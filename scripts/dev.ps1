$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
if (Test-Path -LiteralPath '.tools\cargo\bin\cargo.exe') {
    $env:RUSTUP_HOME = Join-Path $PWD '.tools\rustup'
    $env:CARGO_HOME = Join-Path $PWD '.tools\cargo'
    $env:PATH = "$env:CARGO_HOME\bin;$env:PATH"
}
$env:CALLNOTES_DATA_DIR = Join-Path $PWD '.local-data'
$env:CALLNOTES_ENGINE_DIR = Join-Path $PWD '.tools\engine'
npm.cmd run tauri -- dev
exit $LASTEXITCODE
