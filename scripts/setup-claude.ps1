$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Set-Location (Split-Path $PSScriptRoot -Parent)
$downloadBase = 'https://downloads.claude.ai/claude-code-releases'
$version = (Invoke-RestMethod -Uri "$downloadBase/latest").Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'Versione Claude Code non valida' }
$platform = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'win32-arm64' } else { 'win32-x64' }
$manifest = Invoke-RestMethod -Uri "$downloadBase/$version/manifest.json"
$expected = $manifest.platforms.$platform.checksum
if (!$expected) { throw 'Checksum ufficiale non disponibile' }
$engineDir = Join-Path $PWD '.tools\engine'
New-Item -ItemType Directory -Path $engineDir -Force | Out-Null
$binary = Join-Path $engineDir 'claude.exe'
$temporary = Join-Path $engineDir 'claude.exe.part'
Invoke-WebRequest -Uri "$downloadBase/$version/$platform/claude.exe" -OutFile $temporary
$actual = (Get-FileHash -LiteralPath $temporary -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw 'Checksum Claude Code non corrispondente: file non attivato' }
Move-Item -LiteralPath $temporary -Destination $binary -Force
Write-Output "Claude Code $version pronto nel progetto."
Write-Output 'Per autenticarti: .\.tools\engine\claude.exe auth login'
