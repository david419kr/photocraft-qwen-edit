param([Parameter(Mandatory=$true)][string]$Manifest, [switch]$Silent)
$ErrorActionPreference = 'Stop'
$m = Get-Content -LiteralPath $Manifest -Raw -Encoding UTF8 | ConvertFrom-Json
$backup = Join-Path $m.directory 'original.exe'
$moved = $false
$installed = $false
$preserve = $false
$exited = $false
function Quote-Argument([string]$value) {
    # Windows CommandLineToArgvW quoting, including backslashes before quotes/end.
    '"' + ([regex]::Replace([regex]::Replace($value, '(\\*)"', '$1$1\"'), '(\\+)$', '$1$1')) + '"'
}
function Relaunch {
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $m.target
    $info.WorkingDirectory = $m.cwd
    $info.UseShellExecute = $false
    $info.Arguments = (($m.arguments | ForEach-Object { Quote-Argument $_ }) -join ' ')
    [System.Diagnostics.Process]::Start($info) | Out-Null
}
[IO.File]::WriteAllText((Join-Path $m.directory 'ready'), 'ready')
try {
    # Nothing is moved while the original process still has the executable open.
    $old = Get-Process -Id $m.pid -ErrorAction SilentlyContinue
    if ($old -and -not $old.WaitForExit(120000)) { throw 'PhotoCraft did not exit; update cancelled.' }
    $exited = $true
    if ((Get-FileHash -LiteralPath $m.staged -Algorithm SHA256).Hash.ToLowerInvariant() -ne $m.sha256) { throw 'Staged executable checksum changed.' }
    Move-Item -LiteralPath $m.target -Destination $backup
    $moved = $true
    Move-Item -LiteralPath $m.staged -Destination $m.target
    $installed = $true
    Relaunch
} catch {
    $failure = $_.Exception.Message
    Write-Warning ("PhotoCraft update failed: " + $failure)
    # Keep the original recoverable even if permissions prevent restoration.
    if ($moved) {
        try {
            if ($installed) { Remove-Item -LiteralPath $m.target -Force }
            Move-Item -LiteralPath $backup -Destination $m.target
            $moved = $false
            Relaunch
        } catch {
            # Do not delete the only surviving original executable.
            $preserve = $true
        }
    } elseif ($exited) {
        try { Relaunch } catch { }
    }
    if (!$Silent) { try {
        Add-Type -AssemblyName System.Windows.Forms
        $message = 'PhotoCraft update failed: ' + $failure
        if ($preserve) {
            $recovery = if (Test-Path -LiteralPath $backup) { $backup } else { $m.target }
            $message += "`nOriginal executable retained at: " + $recovery
        }
        [System.Windows.Forms.MessageBox]::Show($message, 'PhotoCraft update') | Out-Null
    } catch { } }
} finally {
    if (!$preserve) {
        # Success and successful rollback remove the ZIP, helper, manifest and backup.
        Remove-Item -LiteralPath $m.directory -Recurse -Force
    }
}
