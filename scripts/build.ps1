param([switch]$Installer, [string]$BuildPython, [string]$UpdaterConfig)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
if ($UpdaterConfig -and !$Installer) { throw 'UpdaterConfig richiede -Installer.' }
if ($UpdaterConfig) {
    $taskUpdater = Get-Content -LiteralPath $UpdaterConfig -Raw | ConvertFrom-Json
    if (!$taskUpdater.pubkey -or !$taskUpdater.endpoints) { throw 'La configurazione updater richiede pubkey ed endpoints.' }
    if (!$env:TAURI_SIGNING_PRIVATE_KEY) { throw 'Configura TAURI_SIGNING_PRIVATE_KEY per firmare gli aggiornamenti.' }
}
if (Test-Path -LiteralPath '.tools\cargo\bin\cargo.exe') {
    $env:RUSTUP_HOME = Join-Path $PWD '.tools\rustup'
    $env:CARGO_HOME = Join-Path $PWD '.tools\cargo'
    $env:PATH = "$env:CARGO_HOME\bin;$env:PATH"
}
& (Join-Path $PSScriptRoot 'bundle-python.ps1') -Platform win-x64
if ($Installer) {
    if (!$BuildPython) {
        $taskLocalPython = Join-Path $PWD '.tools/engine/venv/Scripts/python.exe'
        if (Test-Path -LiteralPath $taskLocalPython -PathType Leaf) { $BuildPython = $taskLocalPython }
        else { $BuildPython = (Get-Command python.exe -ErrorAction Stop).Source }
    }
    & $BuildPython (Join-Path $PSScriptRoot 'bundle-engine.py')
    if ($LASTEXITCODE -ne 0) { throw 'Preparazione dipendenze incluse non riuscita.' }
    & $BuildPython (Join-Path $PSScriptRoot 'prepare-runtime.py') --platform win-x64 --wheelhouse (Join-Path $PWD 'engine/runtime/wheels/win-x64')
    if ($LASTEXITCODE -ne 0) { throw 'Preparazione del motore pronto non riuscita.' }
    & $BuildPython (Join-Path $PSScriptRoot 'prepare-runtime.py') --platform win-x64 --kind documents
    if ($LASTEXITCODE -ne 0) { throw 'Preparazione strumenti documenti e slideshow non riuscita.' }
}
$taskManifest = Get-Content -LiteralPath 'engine/python-runtime.json' -Raw | ConvertFrom-Json
$taskArtifact = $taskManifest.artifacts | Where-Object { $_.platform -eq 'win-x64' } | Select-Object -First 1
$taskResources = @{}
$taskDocuments = Join-Path $PWD 'engine/runtime/documents/win-x64'
if (Test-Path -LiteralPath (Join-Path $taskDocuments 'manifest.json')) { $taskResources[$taskDocuments] = 'engine/runtime/documents/win-x64' }
$taskPrepared = Join-Path $PWD 'engine/runtime/prepared/win-x64'
if (Test-Path -LiteralPath (Join-Path $taskPrepared 'manifest.json')) { $taskResources[$taskPrepared] = 'engine/runtime/prepared/win-x64' }
else { $taskResources[(Join-Path $PWD "engine/runtime/python/$($taskArtifact.file)")] = "engine/runtime/python/$($taskArtifact.file)" }
$taskWheels = Join-Path $PWD 'engine/runtime/wheels/win-x64'
$taskGpu = Join-Path $PWD 'engine/runtime/win-x64-vulkan'
if (Test-Path -LiteralPath $taskGpu -PathType Container) { $taskResources[$taskGpu] = 'engine/runtime/win-x64-vulkan' }
New-Item -ItemType Directory -Path '.tools' -Force | Out-Null
$taskConfig = Join-Path $PWD '.tools/tauri-windows-build.json'
$taskBuildConfig = @{ bundle = @{ active = [bool]$Installer; targets = @('nsis'); resources = $taskResources } }
if ($UpdaterConfig) {
    $taskBuildConfig.bundle.createUpdaterArtifacts = $true
    $taskBuildConfig.plugins = @{ updater = $taskUpdater }
}
$taskJson = $taskBuildConfig | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText($taskConfig, $taskJson, [System.Text.UTF8Encoding]::new($false))
if ($Installer) { npm.cmd run tauri -- build --config $taskConfig }
else { npm.cmd run tauri -- build --debug --no-bundle --config $taskConfig }
exit $LASTEXITCODE
