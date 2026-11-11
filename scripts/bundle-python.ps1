param([ValidateSet('win-x64','mac-arm64','mac-x64')][string]$Platform = 'win-x64')
$ErrorActionPreference = 'Stop'
$taskProject = Split-Path $PSScriptRoot -Parent
$taskManifest = Get-Content -LiteralPath (Join-Path $taskProject 'engine/python-runtime.json') -Raw | ConvertFrom-Json
$taskArtifact = $taskManifest.artifacts | Where-Object { $_.platform -eq $Platform } | Select-Object -First 1
$taskFolder = Join-Path $taskProject 'engine/runtime/python'
New-Item -ItemType Directory -Path $taskFolder -Force | Out-Null
$taskTarget = Join-Path $taskFolder $taskArtifact.file
function Test-PythonArchive([string]$Archive) {
    if (!(Test-Path -LiteralPath $Archive -PathType Leaf)) { return $false }
    if ((Get-Item -LiteralPath $Archive).Length -ne $taskArtifact.size) { return $false }
    $taskStream = [System.IO.File]::OpenRead($Archive)
    $taskHasher = [System.Security.Cryptography.SHA256]::Create()
    try {
        $taskHash = [System.BitConverter]::ToString($taskHasher.ComputeHash($taskStream)).Replace('-', '').ToLowerInvariant()
        return $taskHash -eq $taskArtifact.sha256
    } finally {
        $taskHasher.Dispose()
        $taskStream.Dispose()
    }
}
if (!(Test-PythonArchive $taskTarget)) {
    $taskPartial = "$taskTarget.$([guid]::NewGuid()).part"
    try {
        Write-Host "Download Python $($taskManifest.version) per $Platform..."
        Invoke-WebRequest -UseBasicParsing -Uri $taskArtifact.url -OutFile $taskPartial
        if (!(Test-PythonArchive $taskPartial)) { throw 'Checksum o dimensione del runtime Python non validi.' }
        Move-Item -LiteralPath $taskPartial -Destination $taskTarget -Force
    } finally {
        if (Test-Path -LiteralPath $taskPartial) { Remove-Item -LiteralPath $taskPartial -Force }
    }
}
Write-Host "Python verificato e pronto da includere: $taskTarget"
