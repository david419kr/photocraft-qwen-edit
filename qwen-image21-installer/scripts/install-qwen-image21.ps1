# SPDX-License-Identifier: MIT
param(
    [string]$InstallerRoot = '',
    [string]$PortableRoot = '',
    [string]$Models = '',
    [ValidateSet('', 'Yes', 'No')][string]$DownloadShared = '',
    [string]$TranslationModels = ''
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$headers = @{'User-Agent'='PhotoCraft-Qwen-Setup'}
$comfyVersion = 'v0.39.0'

function Show-Section([string]$Title) {
    Write-Host ''
    Write-Host ('=' * 64) -ForegroundColor DarkCyan
    Write-Host ('  ' + $Title) -ForegroundColor Cyan
    Write-Host ('=' * 64) -ForegroundColor DarkCyan
    Write-Host ''
}

function New-InstallerShortcuts {
    Show-Section 'Ready to use'
    $shell = New-Object -ComObject WScript.Shell
    $launch = Join-Path $script:PortableRoot 'Run-Qwen-Image21.cmd'
    $link = $shell.CreateShortcut((Join-Path $script:InstallerRoot '0. Run-Qwen-Image21.lnk'))
    $link.TargetPath = $launch
    $link.WorkingDirectory = $script:PortableRoot
    $link.Description = 'Start Qwen Image 2.1 (no browser auto-launch)'
    $link.IconLocation = (Join-Path $script:PortableRoot 'python_embeded/python.exe') + ',0'
    $link.Save()
    $number = 1
    foreach ($name in @('diffusion_models', 'text_encoders', 'vae')) {
        $target = Join-Path $script:PortableRoot ('ComfyUI/models/' + $name)
        New-Item -ItemType Directory -Force -Path $target | Out-Null
        $link = $shell.CreateShortcut((Join-Path $script:InstallerRoot ($number.ToString() + '. ' + $name + '.lnk')))
        $link.TargetPath = $target
        $link.WorkingDirectory = $target
        $link.IconLocation = (Join-Path $env:SystemRoot 'System32/shell32.dll') + ',3'
        $link.Save()
        $number++
    }
    [Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell) | Out-Null
    foreach ($name in @('Run-Qwen-Image21', 'diffusion_models', 'text_encoders', 'vae')) {
        $old = Join-Path $script:InstallerRoot ($name + '.lnk')
        if (Test-Path -LiteralPath $old) {
            $backup = Join-Path $script:InstallerRoot 'scripts/legacy-shortcuts'
            Assert-ManagedPath $old -Root $script:InstallerRoot
            Assert-ManagedPath $backup -Root $script:InstallerRoot
            New-Item -ItemType Directory -Force -Path $backup | Out-Null
            $destination = Join-Path $backup ($name + '-' + [Guid]::NewGuid().ToString('N') + '.lnk')
            Assert-ManagedPath $destination -Root $script:InstallerRoot
            Move-Item -LiteralPath $old -Destination $destination
        }
    }
    Write-Host 'Shortcuts created in the installer folder:'
    Write-Host ('  0. Run-Qwen-Image21.lnk -> ' + $launch)
    Write-Host '  1. diffusion_models.lnk | 2. text_encoders.lnk | 3. vae.lnk'
    Write-Host ''
    Write-Host 'Start the Run shortcut. The browser will not open automatically.' -ForegroundColor Green
}

function Assert-ManagedPath([string]$Path, [string]$Root = $script:PortableRoot) {
    $full = [IO.Path]::GetFullPath($Path)
    if (!$full.StartsWith($Root.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Installer path is outside the selected root.' }
    $parent = $full
    while ($parent) {
        if ((Test-Path -LiteralPath $parent) -and ((Get-Item -LiteralPath $parent -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Installer staging folders must not be junctions or symbolic links.' }
        if ($parent -eq $Root) { break }
        $parent = Split-Path -Parent $parent
    }
}

function Download-Verified($Asset, [string]$Target) {
    $hash = ([string]$Asset.digest).Replace('sha256:', '')
    if ($hash -notmatch '^[0-9a-fA-F]{64}$') { throw 'Release asset has no SHA256 digest.' }
    if ((Test-Path -LiteralPath $Target) -and (Get-FileHash -LiteralPath $Target -Algorithm SHA256).Hash -eq $hash) { return }
    $partial = $Target + '.part'
    if (!(Test-Path -LiteralPath $partial) -or (Get-Item -LiteralPath $partial).Length -ne $Asset.size) {
        Write-Host ('Downloading ' + $Asset.name)
        $curl = Get-Command curl.exe -ErrorAction SilentlyContinue
        if ($curl) {
            & $curl.Source --fail --location --retry 3 --retry-delay 2 --connect-timeout 30 --continue-at - --output $partial $Asset.browser_download_url
            if ($LASTEXITCODE -eq 33) {
                Write-Host 'The server cannot resume this archive; starting a fresh download.'
                if (Test-Path -LiteralPath $partial) { Move-Item -LiteralPath $partial -Destination ($partial + '.unresumable-' + [Guid]::NewGuid().ToString('N')) }
                & $curl.Source --fail --location --retry 3 --retry-delay 2 --connect-timeout 30 --output $partial $Asset.browser_download_url
            }
            if ($LASTEXITCODE -ne 0) { throw 'Download interrupted. Rerun this installer to resume.' }
        } else {
            Invoke-WebRequest -Uri $Asset.browser_download_url -OutFile $partial -UseBasicParsing -Headers $script:headers
        }
    }
    if ((Get-Item -LiteralPath $partial).Length -ne $Asset.size -or (Get-FileHash -LiteralPath $partial -Algorithm SHA256).Hash -ne $hash) {
        # Keep corrupt data apart from resumable downloads, rather than retrying a complete bad file forever.
        Move-Item -LiteralPath $partial -Destination ($partial + '.bad-' + [Guid]::NewGuid().ToString('N'))
        throw 'Download checksum mismatch. Rerun to download a fresh copy.'
    }
    Move-Item -LiteralPath $partial -Destination $Target -Force
}

function Get-7Zip64([string]$Tools) {
    $engineRoot = Join-Path $Tools '7zip-26.04'
    $engine = Join-Path $engineRoot 'x64/7za.exe'
    $engineHash = '15d4c788c148e3677e2fc1c4a01f191fb6669eba4f6acf8a465f0f1a6f1e1260'
    if (!(Test-Path -LiteralPath $engine) -or (Get-FileHash -LiteralPath $engine).Hash -ne $engineHash) {
        # The small bootstrap only unpacks the 1.7 MB tool bundle, never ComfyUI.
        $bootstrapAsset = [pscustomobject]@{
            name='7zr.exe'; size=602624
            digest='sha256:256feca8e274e5da655e2a284fabafd9f554365eb164862089dacd4e8276d282'
            browser_download_url='https://github.com/ip7z/7zip/releases/download/26.04/7zr.exe'
        }
        $extraAsset = [pscustomobject]@{
            name='7z2604-extra.7z'; size=1764616
            digest='sha256:dc4b11d3399db18b063630137145f5585d8f7ac847bf3639bd1185d7d1f7cee0'
            browser_download_url='https://github.com/ip7z/7zip/releases/download/26.04/7z2604-extra.7z'
        }
        $bootstrap = Join-Path $Tools '7zr.exe'
        $extra = Join-Path $Tools $extraAsset.name
        Download-Verified $bootstrapAsset $bootstrap
        Download-Verified $extraAsset $extra
        Assert-ManagedPath $engineRoot
        New-Item -ItemType Directory -Force -Path $engineRoot | Out-Null
        & $bootstrap x $extra ('-o' + $engineRoot) -y -bsp0 -bso0
        if ($LASTEXITCODE -ne 0) { throw 'Could not unpack the 64-bit 7-Zip tools.' }
    }
    if ((Get-FileHash -LiteralPath $engine).Hash -ne $engineHash) { throw '64-bit 7-Zip checksum mismatch.' }
    $bytes = [IO.File]::ReadAllBytes($engine)
    $header = [BitConverter]::ToInt32($bytes, 60)
    if ([BitConverter]::ToUInt16($bytes, $header + 4) -ne 0x8664) { throw 'The extractor is not Windows x64.' }
    return $engine
}

function Get-ExtractionResources {
    $system = Get-CimInstance Win32_ComputerSystem
    $memory = Get-CimInstance Win32_OperatingSystem
    $threads = [Math]::Max(1, [int]$system.NumberOfLogicalProcessors)
    $budget = [Math]::Floor([Math]::Min([double]$memory.FreePhysicalMemory * 0.8, [double]$memory.TotalVisibleMemorySize * 0.8) / 1024)
    return [pscustomobject]@{Threads=$threads; MemoryMiB=[Math]::Max(1, [long]$budget)}
}

try {
    if (![Environment]::Is64BitProcess -or $env:PROCESSOR_ARCHITECTURE -ne 'AMD64') { throw 'Windows x64 is required.' }
    if (!$InstallerRoot) {
        $parent = Split-Path -Parent $PSScriptRoot
        $InstallerRoot = if ((Split-Path -Leaf $PSScriptRoot) -eq 'scripts' -and (Test-Path -LiteralPath (Join-Path $parent 'install-qwen-image21.cmd'))) { $parent } else { $PSScriptRoot }
    }
    $InstallerRoot = [IO.Path]::GetFullPath($InstallerRoot)
    $locationFile = Join-Path $InstallerRoot 'scripts/install-location.json'
    Show-Section 'Qwen Image 2.1 - Portable Installer'
    Write-Host 'Official CUDA 13 ComfyUI | Optional SageAttention | Optional translation'
    if (!$PortableRoot) {
        $default = if (Test-Path -LiteralPath (Join-Path $InstallerRoot 'python_embeded/python.exe')) {
            $InstallerRoot
        } else { Join-Path $InstallerRoot 'ComfyUI_windows_portable' }
        if (Test-Path -LiteralPath $locationFile) {
            $remembered = (Get-Content -LiteralPath $locationFile -Raw | ConvertFrom-Json).PortableRoot
            if ($remembered -and (Test-Path -LiteralPath (Join-Path $remembered 'python_embeded/python.exe'))) { $default = $remembered }
        }
        Write-Host ''
        $answer = Read-Host "Install folder [$default]"
        $PortableRoot = if ($answer.Trim()) { $answer.Trim().Trim('"') } else { $default }
    }
    $PortableRoot = [IO.Path]::GetFullPath($PortableRoot)
    if ($PortableRoot -eq [IO.Path]::GetPathRoot($PortableRoot)) { throw 'Choose a dedicated folder, not a drive root.' }
    New-Item -ItemType Directory -Force -Path $PortableRoot | Out-Null
    $python = Join-Path $PortableRoot 'python_embeded/python.exe'
    $main = Join-Path $PortableRoot 'ComfyUI/main.py'
    $tools = Join-Path $PortableRoot '.qwen-setup'
    Assert-ManagedPath $tools
    New-Item -ItemType Directory -Force -Path $tools | Out-Null
    Show-Section '[1/5] Official ComfyUI portable'
    if (!(Test-Path -LiteralPath $python) -or !(Test-Path -LiteralPath $main)) {
        $foreign = @(Get-ChildItem -LiteralPath $PortableRoot -Force | Where-Object { $_.Name -notin @('.qwen-setup','python_embeded','ComfyUI') })
        if ($foreign.Count) { throw 'The chosen folder contains unrelated files. Choose an empty folder or an existing ComfyUI portable root.' }
        Write-Host ('Fetching official ComfyUI CUDA 13 portable ' + $comfyVersion + '.')
        $release = Invoke-RestMethod ('https://api.github.com/repos/Comfy-Org/ComfyUI/releases/tags/' + $comfyVersion) -Headers $headers
        if ($release.tag_name -ne $comfyVersion) { throw 'Unexpected ComfyUI release version.' }
        $asset = @($release.assets | Where-Object { $_.name -eq 'ComfyUI_windows_portable_nvidia.7z' })
        if ($asset.Count -ne 1) { throw 'Official NVIDIA portable download was not found.' }
        $drive = New-Object IO.DriveInfo([IO.Path]::GetPathRoot($PortableRoot))
        if ($drive.AvailableFreeSpace -lt 20GB) { throw 'At least 20 GiB free space is required to download and extract ComfyUI.' }
        $archive = Join-Path $tools $asset[0].name
        Download-Verified $asset[0] $archive
        $seven = Get-7Zip64 $tools
        $resources = Get-ExtractionResources
        $stage = Join-Path $tools 'extract'
        Assert-ManagedPath $stage
        New-Item -ItemType Directory -Force -Path $stage | Out-Null
        Write-Host ('Extracting with 64-bit 7-Zip | Workers: ' + $resources.Threads + ' | RAM budget: ' + [Math]::Round($resources.MemoryMiB / 1024, 1) + ' GiB')
        & $seven x $archive ('-o' + $stage) ('-mmt' + $resources.Threads) ('-mmemuse=' + $resources.MemoryMiB + 'm') -y -bsp1 -bso0
        if ($LASTEXITCODE -ne 0) { throw 'Portable extraction failed. Rerun to retry.' }
        $source = Join-Path $stage 'ComfyUI_windows_portable'
        if (!(Test-Path -LiteralPath (Join-Path $source 'python_embeded/python.exe')) -or !(Test-Path -LiteralPath (Join-Path $source 'ComfyUI/main.py'))) {
            throw 'Unexpected official portable archive layout.'
        }
        # Move fresh directories on the same volume; merge only when recovering a partial install.
        Get-ChildItem -LiteralPath $source -Force | ForEach-Object {
            $destination = [IO.Path]::GetFullPath((Join-Path $PortableRoot $_.Name))
            $origin = [IO.Path]::GetFullPath($_.FullName)
            Assert-ManagedPath $origin
            Assert-ManagedPath $destination
            $prefix = $PortableRoot.TrimEnd('\') + '\'
            if (!$destination.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase) -or !$origin.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid extraction move path.' }
            if (Test-Path -LiteralPath $destination) { Copy-Item -LiteralPath $origin -Destination $PortableRoot -Recurse -Force }
            else { Move-Item -LiteralPath $origin -Destination $destination }
        }
        $release | Select-Object tag_name,published_at | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $tools 'official-release.json') -Encoding UTF8
        # Exact paths remain inside the chosen portable root. Only installer staging/cache are removed.
        $stageFull = [IO.Path]::GetFullPath($stage)
        Assert-ManagedPath $stageFull
        if (!$stageFull.StartsWith($PortableRoot.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid staging path.' }
        Remove-Item -LiteralPath $stageFull -Recurse -Force
        Remove-Item -LiteralPath $archive
    }
    else { Write-Host ('Reusing existing portable: ' + $PortableRoot) }
    $support = Join-Path $PortableRoot 'scripts'
    Assert-ManagedPath $support
    New-Item -ItemType Directory -Force -Path $support | Out-Null
    foreach ($file in @('install-qwen-image21.ps1','qwen-image21-setup.py','install-comfyui-hymt2.ps1')) {
        $from = Join-Path $PSScriptRoot $file
        $to = Join-Path $support $file
        if (!(Test-Path -LiteralPath $from)) { throw "Installer bundle file is missing: $file" }
        if ([IO.Path]::GetFullPath($from) -ne [IO.Path]::GetFullPath($to)) { Copy-Item -LiteralPath $from -Destination $to -Force }
    }
    $from = Join-Path $InstallerRoot 'install-qwen-image21.cmd'
    $to = Join-Path $PortableRoot 'install-qwen-image21.cmd'
    if ([IO.Path]::GetFullPath($from) -ne [IO.Path]::GetFullPath($to)) { Copy-Item -LiteralPath $from -Destination $to -Force }
    $arguments = @('-s','-X','utf8',(Join-Path $support 'qwen-image21-setup.py'),'install','--root',$PortableRoot)
    if ($Models) { $arguments += @('--models',$Models) }
    if ($DownloadShared) { $arguments += @('--shared',$DownloadShared.ToLowerInvariant()) }
    if ($TranslationModels) { $arguments += @('--translations',$TranslationModels) }
    & $python @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Qwen setup did not complete. Check the log and rerun.' }
    # Retain the old launcher support until the new launcher has been written successfully.
    foreach ($base in (@($PortableRoot, $InstallerRoot) | Select-Object -Unique)) {
        foreach ($file in @('install-qwen-image21.ps1','qwen-image21-setup.py','install-comfyui-hymt2.ps1')) {
            $legacy = Join-Path $base $file
            $sourceFile = Join-Path $PSScriptRoot $file
            if ((Test-Path -LiteralPath $legacy) -and ([IO.Path]::GetFullPath($legacy) -ne [IO.Path]::GetFullPath($sourceFile))) {
                $backup = if ($base -eq $PortableRoot) { Join-Path $tools 'legacy-support' } else { Join-Path $base 'scripts/legacy' }
                Assert-ManagedPath $legacy -Root $base
                Assert-ManagedPath $backup -Root $base
                New-Item -ItemType Directory -Force -Path $backup | Out-Null
                $destination = Join-Path $backup ($file + '-' + [Guid]::NewGuid().ToString('N'))
                Assert-ManagedPath $destination -Root $base
                Move-Item -LiteralPath $legacy -Destination $destination
            }
        }
    }
    New-InstallerShortcuts
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $locationFile) | Out-Null
    [pscustomobject]@{PortableRoot=$PortableRoot} | ConvertTo-Json | Set-Content -LiteralPath $locationFile -Encoding UTF8
} catch {
    Write-Host ('Installation failed: ' + $_.Exception.Message) -ForegroundColor Red
    exit 1
}
