# SPDX-License-Identifier: MIT
param(
    [Parameter(Mandatory=$true)][ValidatePattern('^\d{6}-(?:(?:[2-9]|[1-9]\d+)-)?\d+\.\d+\.\d+$')][string]$ReleaseId,
    [string]$Executable = ''
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$installer = Join-Path $repo 'qwen-image21-installer'
if (!$Executable) { $Executable = Join-Path $repo 'target/x86_64-pc-windows-msvc/release/photocraft.exe' }
$Executable = [IO.Path]::GetFullPath($Executable)
if (!(Test-Path -LiteralPath $Executable -PathType Leaf)) { throw 'Build the Windows portable executable first.' }
$cargo = Get-Content -LiteralPath (Join-Path $repo 'Cargo.toml') -Raw
$version = [regex]::Match($cargo, '(?s)\[workspace\.package\].*?version\s*=\s*"([^"]+)"').Groups[1].Value
if (!$ReleaseId.EndsWith('-' + $version)) { throw 'Release suffix must match the upstream base version in Cargo.toml.' }
$dist = Join-Path $repo 'dist'
New-Item -ItemType Directory -Force -Path $dist | Out-Null
$stage = Join-Path $dist ('release-' + $ReleaseId + '-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stage | Out-Null
foreach ($folder in @('scripts','licenses')) { New-Item -ItemType Directory -Path (Join-Path $stage $folder) | Out-Null }
Copy-Item -LiteralPath $Executable -Destination (Join-Path $stage 'photocraft.exe')
Copy-Item -LiteralPath (Join-Path $installer 'install-qwen-image21.cmd') -Destination $stage
foreach ($name in @('install-qwen-image21.ps1','qwen-image21-setup.py','install-comfyui-hymt2.ps1')) {
    Copy-Item -LiteralPath (Join-Path $installer ('scripts/' + $name)) -Destination (Join-Path $stage 'scripts')
}
$licenses = [ordered]@{
    'LICENSE-MIT.txt'='LICENSE-MIT'
    'LICENSE-APACHE.txt'='LICENSE-APACHE'
    'NOTICE.txt'='NOTICE'
    'oxideav-MIT.txt'='crates/codecs/licenses/oxideav-MIT.txt'
    'avif-encoder.txt'='crates/codecs/licenses/avif-encoder.txt'
    'OFL-Inter.txt'='assets/fonts/OFL-Inter.txt'
    'OFL-JetBrainsMono.txt'='assets/fonts/OFL-JetBrainsMono.txt'
    'LICENSE-lucide.txt'='assets/icons/LICENSE-lucide.txt'
    'LICENSE-translations.txt'='crates/ui-egui/src/i18n/LICENSE-translations.txt'
    'LICENSE-app-icon.txt'='assets/app-icon/LICENSE.txt'
    'artcraft-brand-terms.txt'='docs/brand/LICENSE-brand.txt'
}
foreach ($entry in $licenses.GetEnumerator()) {
    Copy-Item -LiteralPath (Join-Path $repo $entry.Value) -Destination (Join-Path $stage ('licenses/' + $entry.Key))
}
$utf8 = New-Object Text.UTF8Encoding($false)
$notice = (Get-Content -LiteralPath (Join-Path $stage 'licenses/NOTICE.txt') -Raw).Replace('docs/brand/LICENSE-brand.txt', 'licenses/artcraft-brand-terms.txt')
# Embedded data/code licenses travel inside NOTICE, preserving the 18-file package contract.
foreach ($licensePath in @('assets/dict/LICENSE-SCOWL.txt', 'crates/algo/data/LICENSE-libfacedetection.txt')) {
    $notice = $notice.Replace("See $licensePath.", 'Full license reproduced below.')
    $notice += "`r`n`r`n--- $licensePath ---`r`n" + (Get-Content -LiteralPath (Join-Path $repo $licensePath) -Raw)
}
[IO.File]::WriteAllText((Join-Path $stage 'licenses/NOTICE.txt'), $notice, $utf8)
[IO.File]::WriteAllText((Join-Path $stage 'portable.txt'), "PhotoCraft portable fork $ReleaseId`r`n", $utf8)
$readme = @'
PhotoCraft - Local Qwen AI portable
==================================

REQUIREMENTS
Windows x64, an NVIDIA RTX GPU, and a CUDA 13 compatible NVIDIA driver.
Internet access is needed for the first installation/model downloads.

SETUP
1. Extract the ZIP into a writable folder; do not run files inside the ZIP.
2. Run install-qwen-image21.cmd.
3. Press Enter to use the displayed default folder, or enter a different path.
4. Answer all model/translation questions at the start, then wait for completion.
   Standard Qwen: 25 steps; Turbo: 8 steps. Encoder/VAE download with the model.
   Optional translation: 1.8B RECOMMENDED for lower VRAM use and coexistence with
   the image model. 7B uses more VRAM and may need unloading/offloading.
5. Start the new "0. Run-Qwen-Image21" shortcut, then run photocraft.exe.
   The launcher does not open a browser. Default ComfyUI address: 127.0.0.1:8188.
Rerun the installer to add models or translation later. Existing models are kept.

AI QUICK GUIDE
- Blank transparent/pure-white canvas, no selection/Select All: Prompt Generate.
- Existing image, no partial selection: Prompt Edit. Results are new layers;
  whole-image operations hide the source layers. Background Removal is one click.
- Partial selection: Generative Fill. Blank prompt or Remove erases the region.
- The language icon translates prompts to English; its setting is remembered.
- Ctrl+Enter generates; the rotate-arrow button rerolls with a new seed.
- The info button shows ordered prompts, translations and approximate fill areas.
  Undo/Redo also updates prompt history. Save in PNG/AVIF/PSD/pcraft to retain it.
  For Export As, choose Metadata: All. Metadata: None removes prompt history.
- AI controls appear only when the required service/models are detected.

AVIF opening, saving and export are included; exports currently target SDR.
Original upstream shortcuts and export defaults are retained. The last selected
tool is restored at startup.
Models, ComfyUI, personal settings and private documents are not bundled here.

SOURCE AND SCREENSHOT GUIDES
https://github.com/david419kr/photocraft-qwen-edit
English: README.md in the repository; Korean: README.ko.md.
Licenses and original attribution are in the licenses folder.
'@
$readme += "`r`nRelease: $ReleaseId`r`nUpstream base: PhotoCraft $version`r`nThe date tracks this fork; the version suffix tracks upstream.`r`n"
[IO.File]::WriteAllText((Join-Path $stage 'README.txt'), $readme, $utf8)
$zipPath = Join-Path $dist ('photocraft-qwen-edit-' + $ReleaseId + '-windows-x64.zip')
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$stream = [IO.File]::Open($zipPath, [IO.FileMode]::Create)
$zip = New-Object IO.Compression.ZipArchive($stream, ([IO.Compression.ZipArchiveMode]::Create))
try {
    foreach ($file in Get-ChildItem -LiteralPath $stage -Recurse -File) {
        $relative = $file.FullName.Substring($stage.Length + 1).Replace('\','/')
        [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $file.FullName, $relative, [IO.Compression.CompressionLevel]::Optimal) | Out-Null
    }
} finally { $zip.Dispose(); $stream.Dispose() }
$sumPath = $zipPath + '.sha256'
[IO.File]::WriteAllText($sumPath, ((Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant() + '  ' + [IO.Path]::GetFileName($zipPath) + "`n"), $utf8)
Write-Output "Stage: $stage"
Write-Output "ZIP: $zipPath"
Write-Output "SHA256: $sumPath"
