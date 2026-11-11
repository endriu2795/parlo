$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$env:CALLNOTES_ENGINE_DIR = Join-Path $PWD '.tools\engine'
$env:CALLNOTES_DATA_DIR = Join-Path $PWD '.local-data'
$taskCandidates = @('src-tauri\target\release\callnotes.exe', 'src-tauri\target\debug\callnotes.exe', 'src-tauri\target\staged\callnotes.exe', 'src-tauri\target\hardware\callnotes.exe')
$taskBuilt = $taskCandidates | ForEach-Object { $taskPath = Join-Path $PWD $_; if (Test-Path -LiteralPath $taskPath) { Get-Item -LiteralPath $taskPath } } | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (!$taskBuilt) { throw 'Prima compila l’app con npm run desktop:build' }
$binary = $taskBuilt.FullName
Start-Process -FilePath $binary -WorkingDirectory $PWD -WindowStyle Hidden
