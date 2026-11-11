param([string]$Python = 'python', [ValidateSet('turbo','tiny','small')][string]$Model = 'turbo')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
New-Item -ItemType Directory -Path '.tools\engine' -Force | Out-Null
& $Python -m venv '.tools\engine\venv'
if ($LASTEXITCODE -ne 0) { throw 'Creazione ambiente Python fallita' }
$enginePython = Join-Path $PWD '.tools\engine\venv\Scripts\python.exe'
$requirements = if (Test-Path -LiteralPath 'engine\requirements-lock.txt') { 'engine/requirements-lock.txt' } else { 'engine/requirements.txt' }
& $enginePython -m pip install --disable-pip-version-check -r $requirements
if ($LASTEXITCODE -ne 0) { throw 'Installazione motore fallita' }
& $enginePython scripts/setup-engine.py --model $Model
if ($LASTEXITCODE -ne 0) { throw 'Download modelli fallito' }
