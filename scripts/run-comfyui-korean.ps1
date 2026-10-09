param(
    [switch]$DebugBuild,
    [switch]$NoBuild,
    [switch]$NoRun
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$profileName = if ($DebugBuild) { 'debug' } else { 'release' }
$targetTriple = 'x86_64-pc-windows-msvc'
$targetFlagsKey = 'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS'
$previousTargetFlags = [Environment]::GetEnvironmentVariable($targetFlagsKey)
$bundleDir = Join-Path $repoRoot 'dist\photocraft-comfyui-korean'
$dataDir = Join-Path $bundleDir 'PhotoCraftData'

Push-Location $repoRoot
try {
    if (-not $NoBuild) {
        # Match official Windows packaging: include the CRT in the executable.
        [Environment]::SetEnvironmentVariable($targetFlagsKey, ($previousTargetFlags + ' -C target-feature=+crt-static').Trim())
        $cargoArgs = @('+1.95.0', 'build', '-p', 'photocraft', '--target', $targetTriple)
        if (-not $DebugBuild) { $cargoArgs += '--release' }
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) { throw ('Build failed: ' + $LASTEXITCODE) }
    }
    $builtExe = Join-Path $repoRoot ('target\' + $targetTriple + '\' + $profileName + '\photocraft.exe')
    if (-not (Test-Path -LiteralPath $builtExe)) { throw ('Executable not found: ' + $builtExe) }
    New-Item -ItemType Directory -Path $dataDir -Force | Out-Null
    Copy-Item -LiteralPath $builtExe -Destination (Join-Path $bundleDir 'photocraft.exe') -Force
    $licenseDir = Join-Path $bundleDir 'licenses'
    New-Item -ItemType Directory -Path $licenseDir -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $repoRoot 'crates\codecs\licenses\oxideav-MIT.txt') -Destination $licenseDir -Force
    Copy-Item -LiteralPath (Join-Path $repoRoot 'crates\codecs\licenses\avif-encoder.txt') -Destination $licenseDir -Force
    # Remove only the runtime bundled by the previous development launcher.
    $oldRuntime = Join-Path $bundleDir 'VCRUNTIME140.dll'
    if (Test-Path -LiteralPath $oldRuntime -PathType Leaf) {
        Remove-Item -LiteralPath $oldRuntime
    }
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
    [Environment]::SetEnvironmentVariable($targetFlagsKey, $previousTargetFlags)
    Pop-Location
}
