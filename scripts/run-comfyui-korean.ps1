param(
    [switch]$DebugBuild,
    [switch]$NoBuild,
    [switch]$NoRun
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$profileName = if ($DebugBuild) { 'debug' } else { 'release' }
$bundleDir = Join-Path $repoRoot 'dist\photocraft-comfyui-korean'
$dataDir = Join-Path $bundleDir 'PhotoCraftData'

Push-Location $repoRoot
try {
    if (-not $NoBuild) {
        $cargoArgs = @('+1.95.0', 'build', '-p', 'photocraft')
        if (-not $DebugBuild) { $cargoArgs += '--release' }
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) { throw ('Build failed: ' + $LASTEXITCODE) }
    }
    $builtExe = Join-Path $repoRoot ('target\' + $profileName + '\photocraft.exe')
    if (-not (Test-Path -LiteralPath $builtExe)) { throw ('Executable not found: ' + $builtExe) }
    New-Item -ItemType Directory -Path $dataDir -Force | Out-Null
    Copy-Item -LiteralPath $builtExe -Destination (Join-Path $bundleDir 'photocraft.exe') -Force
    [IO.File]::WriteAllText((Join-Path $bundleDir 'portable.txt'), 'PhotoCraft ComfyUI development build')
    $prefsPath = Join-Path $dataDir 'preferences.json'
    if (-not (Test-Path -LiteralPath $prefsPath)) {
        [IO.File]::WriteAllText($prefsPath, '{"interface":{"language":"ko"},"integrations":{"comfyuiUrl":"http://127.0.0.1:8188"}}')
    }
    Write-Output ('PhotoCraft: ' + (Join-Path $bundleDir 'photocraft.exe'))
    if (-not $NoRun) {
        Start-Process -FilePath (Join-Path $bundleDir 'photocraft.exe') -WorkingDirectory $bundleDir
    }
} finally {
    Pop-Location
}
