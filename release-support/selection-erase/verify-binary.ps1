param([Parameter(Mandatory=$true)][string]$Executable)
$ErrorActionPreference = 'Stop'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$dumpbin = Get-ChildItem "$vs/VC/Tools/MSVC/*/bin/Hostx64/x64/dumpbin.exe" | Sort-Object FullName -Descending | Select-Object -First 1
if (!$dumpbin) { throw 'dumpbin missing' }
$headers = & $dumpbin.FullName /headers $Executable
if ($LASTEXITCODE -ne 0 -or ($headers -join "`n") -notmatch '8664 machine \(x64\)') { throw 'Not AMD64 PE' }
$deps = & $dumpbin.FullName /dependents $Executable
if ($LASTEXITCODE -ne 0) { throw 'Cannot read imports' }
$dlls = @($deps | Where-Object { $_ -match '^\s+\S+\.dll\s*$' } | ForEach-Object { $_.Trim() })
if (!$dlls.Count -or ($dlls -match '(?i)^(vcruntime|msvcp|msvcr|ucrtbase|api-ms-win-crt)')) { throw 'Dynamic CRT or no imports' }
$output = "$Executable.version.txt"
$proc = Start-Process -FilePath $Executable -ArgumentList '--version' -Wait -PassThru -NoNewWindow -RedirectStandardOutput $output
if ($proc.ExitCode -ne 0) { throw 'Executable version failed' }
[ordered]@{ version=(Get-Content $output -Raw).Trim(); imported_dlls=$dlls; amd64=$true; static_crt=$true } | ConvertTo-Json -Depth 4
