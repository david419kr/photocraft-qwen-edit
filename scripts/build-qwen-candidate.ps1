# SPDX-License-Identifier: MIT
# Package an already built candidate; never create a GitHub release or a tag.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo
if ($env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS -ne '-C target-feature=+crt-static') {
    throw 'Candidate builds require the static MSVC CRT flag.'
}
$cargo = Get-Content Cargo.toml -Raw
$version = [regex]::Match($cargo, '(?s)\[workspace\.package\].*?version\s*=\s*"([^"]+)"').Groups[1].Value
$tokyo = [TimeZoneInfo]::ConvertTimeBySystemTimeZoneId([DateTimeOffset]::UtcNow, 'Tokyo Standard Time')
$releaseId = $tokyo.ToString('yyMMdd') + '-' + $version
$exe = Join-Path $repo 'target/x86_64-pc-windows-msvc/release/photocraft.exe'
New-Item -ItemType Directory -Force dist | Out-Null
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ($LASTEXITCODE -ne 0 -or !$vs) { throw 'MSVC x64 tools not found.' }
$dumpbin = Get-ChildItem "$vs/VC/Tools/MSVC/*/bin/Hostx64/x64/dumpbin.exe" | Sort-Object FullName -Descending | Select-Object -First 1
if (!$dumpbin) { throw 'dumpbin not found.' }
$headers = & $dumpbin.FullName /headers $exe
if ($LASTEXITCODE -ne 0 -or ($headers -join "`n") -notmatch '8664 machine \(x64\)') { throw 'Not a Windows x64 PE executable.' }
$headers | Set-Content dist/candidate-headers.txt
$dependents = & $dumpbin.FullName /dependents $exe
if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect PE imports.' }
$dependents | Set-Content dist/candidate-dependents.txt
$dlls = @($dependents | Where-Object { $_ -match '^\s+\S+\.dll\s*$' } | ForEach-Object { $_.Trim() })
if (!$dlls.Count) { throw 'No PE imports found; cannot verify the CRT.' }
if ($dlls -match '(?i)^(vcruntime|msvcp|msvcr|ucrtbase|api-ms-win-crt)') { throw 'Dynamic CRT dependency found.' }
# The GUI subsystem requires redirected stdout and an explicit process wait for --version.
$process = Start-Process -FilePath $exe -ArgumentList '--version' -Wait -PassThru -NoNewWindow -RedirectStandardOutput dist/candidate-version.txt
if ($process.ExitCode -ne 0) { throw 'Candidate --version failed.' }
$reported = Get-Content dist/candidate-version.txt -Raw
if ($reported -notmatch [regex]::Escape($version)) { throw 'Candidate version does not match Cargo.toml.' }
& ./scripts/package-qwen-portable.ps1 -ReleaseId $releaseId -Executable $exe
$zip = Join-Path $repo "dist/photocraft-qwen-edit-$releaseId-windows-x64.zip"
python scripts/verify-qwen-portable.py --zip $zip --executable $exe
if ($LASTEXITCODE -ne 0) { throw 'Portable ZIP contract failed.' }
$source = (git rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve source commit.' }
$rust = (rustc +1.95.0 --version).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve Rust version.' }
$metadata = [ordered]@{
    source_sha = $source
    fork_base_sha = $env:QWEN_BASE_SHA
    upstream_tag = 'v0.8.2'
    upstream_release_id = 410949848
    upstream_sha = $env:QWEN_UPSTREAM_SHA
    toolchain = $rust
    target = 'x86_64-pc-windows-msvc'
    rustflags = $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS
    features = 'desktop manifest: avif,avif-decode; no optional heif or craft-fonts'
    executable_sha256 = (Get-FileHash $exe -Algorithm SHA256).Hash.ToLowerInvariant()
    zip_sha256 = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    zip = [IO.Path]::GetFileName($zip)
    imported_dlls = $dlls
    actual_gpu_ui_tested = $false
    ci_run_url = "$($env:GITHUB_SERVER_URL)/$($env:GITHUB_REPOSITORY)/actions/runs/$($env:GITHUB_RUN_ID)"
    built_at_utc = [DateTimeOffset]::UtcNow.ToString('o')
}
$metadata | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 dist/candidate-build.json
# Keep provenance inspectable through the job-log API when artifact transfer is unavailable.
$metadata | ConvertTo-Json -Depth 4 | Write-Output
if ($env:GITHUB_STEP_SUMMARY) {
    @"
### Windows candidate (not a release)
Source: ``$source``
Upstream: ``$($env:QWEN_UPSTREAM_SHA)``
ZIP: ``$([IO.Path]::GetFileName($zip))``
SHA256: ``$($metadata.zip_sha256)``
Actual CUDA, VRAM and interactive UI checks remain local. Artifact retention: 14 days.
"@ | Add-Content $env:GITHUB_STEP_SUMMARY
}
