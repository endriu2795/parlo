param([string]$VulkanSdk = '', [int]$Jobs = 4)
$ErrorActionPreference = 'Stop'
$taskRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location -LiteralPath $taskRoot
if (!$VulkanSdk) { $VulkanSdk = $env:VULKAN_SDK }
if (!$VulkanSdk) { $VulkanSdk = Join-Path $taskRoot '.tools/vulkan-sdk' }
$env:VULKAN_SDK = (Resolve-Path -LiteralPath $VulkanSdk).Path
$taskVulkanInclude = Join-Path $env:VULKAN_SDK 'Include'
$taskVulkanLibrary = Join-Path $env:VULKAN_SDK 'Lib/vulkan-1.lib'
$taskVulkanCompiler = Join-Path $env:VULKAN_SDK 'Bin/glslc.exe'
foreach ($taskSdkFile in @((Join-Path $taskVulkanInclude 'vulkan/vulkan.h'), $taskVulkanLibrary, $taskVulkanCompiler)) {
    if (!(Test-Path -LiteralPath $taskSdkFile -PathType Leaf)) { throw "Vulkan SDK incompleto: manca $taskSdkFile. Installa il SDK ufficiale; l'estrazione dell'installer con 7-Zip non è sufficiente." }
}
$taskVswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$taskVs = & $taskVswhere -latest -products '*' -property installationPath
$taskCmake = Join-Path $taskVs 'Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe'
if (!(Test-Path -LiteralPath $taskCmake)) { $taskCmake = 'cmake' }
$taskSource = Join-Path $taskRoot '.tools/whisper.cpp-1.8.3'
if (!(Test-Path -LiteralPath (Join-Path $taskSource 'CMakeLists.txt'))) { throw 'Scarica prima i sorgenti ufficiali whisper.cpp v1.8.3 in .tools/whisper.cpp-1.8.3.' }
$taskBuild = Join-Path $taskRoot '.tools/whisper-cpp-build'
& $taskCmake -S $taskSource -B $taskBuild -G 'Visual Studio 17 2022' -A x64 "-DVulkan_INCLUDE_DIR:PATH=$taskVulkanInclude" "-DVulkan_LIBRARY:FILEPATH=$taskVulkanLibrary" "-DVulkan_GLSLC_EXECUTABLE:FILEPATH=$taskVulkanCompiler" -DGGML_VULKAN=ON -DGGML_NATIVE=OFF -DGGML_AVX=OFF -DGGML_AVX2=OFF -DGGML_FMA=OFF -DGGML_F16C=OFF -DWHISPER_BUILD_TESTS=OFF -DWHISPER_BUILD_EXAMPLES=ON
if ($LASTEXITCODE -ne 0) { throw 'Configurazione Vulkan fallita' }
& $taskCmake --build $taskBuild --config Release --target whisper-cli --parallel $Jobs
if ($LASTEXITCODE -ne 0) { throw 'Compilazione Vulkan fallita' }
$taskRuntime = Join-Path $taskRoot 'engine/runtime/win-x64-vulkan'
New-Item -ItemType Directory -Path $taskRuntime -Force | Out-Null
Get-ChildItem -LiteralPath (Join-Path $taskBuild 'bin/Release') -File | Where-Object { $_.Extension -eq '.dll' -or $_.Name -eq 'whisper-cli.exe' } | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $taskRuntime -Force }
$taskRedist = Get-ChildItem -LiteralPath (Join-Path $taskVs 'VC/Redist/MSVC') -Directory | Where-Object Name -match '^14\.' | Sort-Object Name -Descending | Select-Object -First 1
foreach ($taskComponent in @('Microsoft.VC143.CRT', 'Microsoft.VC143.OpenMP')) {
    $taskDlls = Join-Path $taskRedist.FullName "x64/$taskComponent"
    Get-ChildItem -LiteralPath $taskDlls -Filter '*.dll' -File | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $taskRuntime -Force }
}
Copy-Item -LiteralPath (Join-Path $taskSource 'LICENSE') -Destination (Join-Path $taskRuntime 'LICENSE-whisper.txt') -Force
@{ backend = 'vulkan'; version = '1.8.3'; platform = 'Windows'; architecture = 'AMD64' } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRuntime 'runtime.json') -Encoding utf8
Write-Output "Runtime portabile pronto: $taskRuntime. Il SDK non serve sui PC degli utenti."
