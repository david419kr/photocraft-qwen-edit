# SPDX-License-Identifier: MIT
param([Alias('ComfyRoot')][string]$PortableRoot = '')
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
$fork = 'https://github.com/david419kr/ComfyUI-Hy-MT2'
$nodeRevision = '22ea3d7ebc8938515c0f60dbcd991edbb46ae8d5'
$uvVersion = '0.10.11'
$uvHash = '9ee74df98582f37fdd6069e1caac80d2616f9a489f5dbb2b1c152f30be69c58e'
$pythonUrl = 'https://github.com/astral-sh/python-build-standalone/releases/download/20260310/cpython-3.13.12%2B20260310-x86_64-pc-windows-msvc-install_only_stripped.tar.gz'
$pythonHash = '09df14cdd7ad59863596189623b2f34f6a2925b9317279ce53346a38dd62e60f'

function Download([string]$Url, [string]$File) {
    Invoke-WebRequest -Uri $Url -OutFile $File -UseBasicParsing -Headers @{'User-Agent'='ComfyUI-Hy-MT2-installer'}
}
function Checked-Path([string]$Path) {
    $full = [IO.Path]::GetFullPath($Path)
    if (!$full.StartsWith($script:PortableRoot.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Path outside the portable installation: $full"
    }
    $parent = $full
    while ($parent) {
        if ((Test-Path -LiteralPath $parent) -and ((Get-Item -LiteralPath $parent -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Managed runtime paths must not be junctions or symbolic links: $parent"
        }
        if ($parent -eq $script:PortableRoot) { break }
        $parent = Split-Path -Parent $parent
    }
    return $full
}
function Run-UV([string[]]$Arguments) {
    & $script:uv @Arguments
    if ($LASTEXITCODE -ne 0) { throw "uv failed: $($Arguments -join ' ')" }
}
function Private-Python([string]$Node, [string]$HostPython, [string]$Tools) {
    $root = Checked-Path (Join-Path $Node '.python')
    New-Item -ItemType Directory -Path $root -Force | Out-Null
    $candidates = @((Join-Path $root 'python/python.exe'))
    # A failed uv install can already have extracted a usable patch-version directory.
    $legacy = Get-ChildItem -LiteralPath $root -Directory | Where-Object {
        $_.Name -match '^cpython-3\.13\.\d+-windows-x86_64-none$' -and !($_.Attributes -band [IO.FileAttributes]::ReparsePoint)
    } | Sort-Object Name -Descending
    foreach ($directory in $legacy) { $candidates += Join-Path $directory.FullName 'python.exe' }
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            $candidate = Checked-Path $candidate
            & $candidate -I -c 'import sys; sys.exit(0 if sys.version_info[:2] == (3, 13) else 1)'
            if ($LASTEXITCODE -eq 0) { return $candidate }
        }
    }
    Write-Host 'Downloading private Python 3.13.12 (no version links).'
    $archive = Checked-Path (Join-Path $Tools ('python-' + [Guid]::NewGuid().ToString('N') + '.tar.gz'))
    Download $script:pythonUrl $archive
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $script:pythonHash) { throw 'Python download checksum mismatch.' }
    $extract = @'
import pathlib, sys, tarfile
root = pathlib.Path(sys.argv[2]).resolve()
with tarfile.open(sys.argv[1], 'r:gz') as archive:
    members = archive.getmembers()
    for member in members:
        target = (root / member.name).resolve()
        if not (member.isfile() or member.isdir()) or not target.is_relative_to(root):
            raise RuntimeError('Python archive contains a link or an unsafe path')
    archive.extractall(root, members=members, filter='data')
'@
    & $HostPython -I -c $extract $archive $root
    if ($LASTEXITCODE -ne 0) { throw 'Private Python archive extraction failed.' }
    $python = Checked-Path (Join-Path $root 'python/python.exe')
    if (!(Test-Path -LiteralPath $python -PathType Leaf)) { throw 'Private Python executable is missing after extraction.' }
    & $python -I -c 'import sys; sys.exit(0 if sys.version_info[:2] == (3, 13) else 1)'
    if ($LASTEXITCODE -ne 0) { throw 'Private Python 3.13 cannot start.' }
    Remove-Item -LiteralPath $archive
    return $python
}

try {
    if (![Environment]::Is64BitProcess -or $env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {
        throw 'This installer requires 64-bit Windows PowerShell on Windows x64.'
    }
    $candidates = if ($PortableRoot) { @($PortableRoot) } else { @($PSScriptRoot, (Get-Location).Path) }
    $found = $null
    foreach ($candidate in $candidates) {
        $candidate = [IO.Path]::GetFullPath($candidate)
        if ((Test-Path -LiteralPath (Join-Path $candidate 'main.py')) -and (Test-Path -LiteralPath (Join-Path (Split-Path -Parent $candidate) 'python_embeded/python.exe'))) {
            $candidate = Split-Path -Parent $candidate
        }
        if ((Test-Path -LiteralPath (Join-Path $candidate 'ComfyUI/main.py')) -and (Test-Path -LiteralPath (Join-Path $candidate 'python_embeded/python.exe'))) {
            $found = $candidate
            break
        }
    }
    if (!$found) { throw 'Put both installer files inside ComfyUI_windows_portable, or pass -PortableRoot <folder>.' }
    $PortableRoot = if ($found -eq [IO.Path]::GetPathRoot($found)) { $found } else { $found.TrimEnd('\') }
    $comfy = Join-Path $PortableRoot 'ComfyUI'
    $hostPython = Join-Path $PortableRoot 'python_embeded/python.exe'
    Write-Host "Portable root: $PortableRoot"
    Write-Host 'Close ComfyUI before installing or updating this node.'
    & $hostPython -I -c 'import torch'
    if ($LASTEXITCODE -ne 0) { throw 'The portable ComfyUI Python cannot import torch. Repair the base ComfyUI installation first.' }

    $tools = Checked-Path (Join-Path $PortableRoot '.hymt2-tools')
    New-Item -ItemType Directory -Path $tools -Force | Out-Null
    $uv = Join-Path $tools 'uv.exe'
    if (!(Test-Path -LiteralPath $uv)) {
        $zip = Join-Path $tools 'uv.zip'
        Download "https://github.com/astral-sh/uv/releases/download/$uvVersion/uv-x86_64-pc-windows-msvc.zip" $zip
        if ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash -ne $uvHash) { throw 'uv download checksum mismatch.' }
        Expand-Archive -LiteralPath $zip -DestinationPath $tools -Force
        if (!(Test-Path -LiteralPath $uv)) { throw 'uv.exe was not found in the downloaded archive.' }
        Remove-Item -LiteralPath $zip
    }

    $nodes = Checked-Path (Join-Path $comfy 'custom_nodes')
    New-Item -ItemType Directory -Path $nodes -Force | Out-Null
    $node = Checked-Path (Join-Path $nodes 'ComfyUI-Hy-MT2')
    $git = Get-Command git -ErrorAction SilentlyContinue
    # Portable drives can report a different owner; trust only this node for these commands.
    $nodeGitArgs = @('-c', ('safe.directory=' + $node.Replace('\','/')), '-C', $node)
    $update = $false
    if ((Test-Path -LiteralPath (Join-Path $node '.git')) -and $git) {
        $origin = & $git.Source @nodeGitArgs remote get-url origin 2>$null
        if ($LASTEXITCODE -eq 0 -and $origin.TrimEnd('/').Replace('.git','') -eq $fork) {
            $dirty = & $git.Source @nodeGitArgs status --porcelain
            if ($LASTEXITCODE -ne 0 -or $dirty) { throw 'The fork has local source changes; commit or back them up before updating.' }
            & $git.Source @nodeGitArgs fetch origin $nodeRevision
            if ($LASTEXITCODE -ne 0) { throw 'Fork update failed; existing files were preserved.' }
            & $git.Source @nodeGitArgs merge-base --is-ancestor HEAD $nodeRevision
            if ($LASTEXITCODE -ne 0) { throw 'The installed fork has a different revision history; back it up before replacing it with the pinned release.' }
            & $git.Source @nodeGitArgs checkout --detach $nodeRevision
            if ($LASTEXITCODE -ne 0) { throw 'Could not select the pinned Hy-MT2 revision.' }
            $update = $true
        }
    }
    if (!$update) {
        $stage = Checked-Path (Join-Path $tools ('node-' + [Guid]::NewGuid().ToString('N')))
        New-Item -ItemType Directory -Path $stage -Force | Out-Null
        if ($git) {
            $source = Checked-Path (Join-Path $stage 'source')
            & $git.Source clone ($fork + '.git') $source
            if ($LASTEXITCODE -ne 0) { throw 'Hy-MT2 fork download failed.' }
            & $git.Source -c ('safe.directory=' + $source.Replace('\','/')) -C $source checkout --detach $nodeRevision
            if ($LASTEXITCODE -ne 0) { throw 'Could not select the pinned Hy-MT2 revision.' }
        } else {
            $archive = Join-Path $stage 'node.zip'
            Download ($fork + '/archive/' + $nodeRevision + '.zip') $archive
            Expand-Archive -LiteralPath $archive -DestinationPath $stage
            $source = Join-Path $stage ('ComfyUI-Hy-MT2-' + $nodeRevision)
        }
        if (!(Test-Path -LiteralPath (Join-Path $source 'runtime/llama_worker.py')) -or !(Test-Path -LiteralPath (Join-Path $source 'py/residency.py'))) { throw 'Downloaded fork is missing its private runtime or residency support.' }
        $backup = $null
        if (Test-Path -LiteralPath $node) {
            $backups = Checked-Path (Join-Path $PortableRoot '.hymt2-backups')
            New-Item -ItemType Directory -Path $backups -Force | Out-Null
            $backup = Checked-Path (Join-Path $backups ('ComfyUI-Hy-MT2-' + [Guid]::NewGuid().ToString('N')))
            Move-Item -LiteralPath $node -Destination $backup
            Write-Host "Previous node backed up: $backup"
        }
        try {
            $source = Checked-Path $source
            Move-Item -LiteralPath $source -Destination $node
        } catch {
            if ($backup -and !(Test-Path -LiteralPath $node)) { Move-Item -LiteralPath $backup -Destination $node }
            throw
        }
        if ($backup -and (Test-Path -LiteralPath (Join-Path $backup '.venv'))) {
            $oldEnv = Checked-Path (Join-Path $backup '.venv')
            $newEnv = Checked-Path (Join-Path $node '.venv')
            Move-Item -LiteralPath $oldEnv -Destination $newEnv
        }
    }

    $oldPythonDir = [Environment]::GetEnvironmentVariable('UV_PYTHON_INSTALL_DIR')
    $oldDownloads = [Environment]::GetEnvironmentVariable('UV_PYTHON_DOWNLOADS')
    try {
        Remove-Item Env:UV_PYTHON_INSTALL_DIR -ErrorAction SilentlyContinue
        $env:UV_PYTHON_DOWNLOADS = 'never'
        $basePython = Private-Python $node $hostPython $tools
        $venv = Checked-Path (Join-Path $node '.venv')
        $python = Join-Path $venv 'Scripts/python.exe'
        $validEnv = $false
        if (Test-Path -LiteralPath $python) {
            & $python -I -c 'import sys; sys.exit(0 if sys.version_info[:2] == (3, 13) else 1)'
            $validEnv = $LASTEXITCODE -eq 0
        }
        if (!$validEnv) {
            if (Test-Path -LiteralPath $venv) {
                $oldEnv = Checked-Path (Join-Path $node ('.venv.backup-' + [Guid]::NewGuid().ToString('N')))
                Move-Item -LiteralPath $venv -Destination $oldEnv
            }
            Run-UV -Arguments @('venv','--no-managed-python','--no-python-downloads','--python',$basePython,'--relocatable','--link-mode','copy',$venv)
        }
        Run-UV -Arguments @('pip','install','--python',$python,'--only-binary',':all:','--link-mode','copy','--requirements',(Join-Path $node 'runtime/requirements.txt'))
        & $hostPython -I -c "import importlib.util,sys; sys.exit(0 if importlib.util.find_spec('huggingface_hub') else 1)"
        if ($LASTEXITCODE -ne 0) { Run-UV -Arguments @('pip','install','--python',$hostPython,'--system','--only-binary',':all:','huggingface-hub') }
        & $python -I -X utf8 (Join-Path $node 'runtime/llama_worker.py') --check
        if ($LASTEXITCODE -ne 0) { throw 'Private runtime validation failed. Check the NVIDIA driver and messages above, then rerun this installer.' }
    } finally {
        [Environment]::SetEnvironmentVariable('UV_PYTHON_INSTALL_DIR', $oldPythonDir)
        [Environment]::SetEnvironmentVariable('UV_PYTHON_DOWNLOADS', $oldDownloads)
    }
    Write-Host "Hy-MT2 fork installed: $node"
    Write-Host "Pinned revision: $nodeRevision (persistent translation and image loader cache)"
    Write-Host "Private Python/CUDA runtime: $venv"
    Write-Host 'Restart ComfyUI. GGUF files belong in ComfyUI/models/LLM/GGUF.'
} catch {
    Write-Host ("Installation failed: " + $_.Exception.Message) -ForegroundColor Red
    exit 1
}
