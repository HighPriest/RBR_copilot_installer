<#
.SYNOPSIS
    AUTOMATED TEST. Proves a panic leaves a report behind. Exits 0 on
    success, 1 on any failed assertion.

.DESCRIPTION
    The crash path is the one piece of error handling that using the
    installer correctly never exercises, and an untested crash handler is
    one that fails the day it is finally needed. This drives
    `--selftest-panic`, which panics on purpose with the window suppressed.

    What it has to prove, because each part failed for real before the hook
    existed (2026-10-26):
      the process dies the way Windows reports it   (0xC0000409)
      a report is written where it was pointed
      the report names the SOURCE FILE AND LINE, which is the whole
        difference between an actionable report and an offset into a
        stripped binary
      with no path given, the report still lands beside the exe

    Needs Windows and the release build; needs no desktop, no server and no
    game folder, so this one runs in CI.

.EXAMPLE
    pwsh -File tests/Test-CrashReport.ps1
#>
[CmdletBinding()]
param([string]$ExePath)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$crateRoot = Split-Path -Parent $PSScriptRoot
if (-not $ExePath) {
    $ExePath = Join-Path $crateRoot 'target\x86_64-pc-windows-msvc\release\pacenotes-installer.exe'
}
if (-not (Test-Path $ExePath)) {
    Write-Error "Installer not built. Run: cargo build --release --target x86_64-pc-windows-msvc"
    exit 1
}
$ExePath = (Resolve-Path $ExePath).Path

$script:failures = New-Object System.Collections.ArrayList
$script:checks = 0
function Assert-That {
    param([Parameter(Mandatory)][string]$Name, [Parameter(Mandatory)][bool]$Condition, [string]$Detail = '')
    $script:checks++
    if ($Condition) { Write-Host "  PASS  $Name" -ForegroundColor Green }
    else {
        Write-Host ("  FAIL  {0}{1}" -f $Name, $(if ($Detail) { " -- $Detail" } else { '' })) -ForegroundColor Red
        [void]$script:failures.Add($Name)
    }
}

Write-Host '=== crash report ===' -ForegroundColor Cyan

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("pacenotes-crash-" + [guid]::NewGuid())
[void](New-Item -ItemType Directory -Force -Path $work)
# The fallback report lands beside the exe, so the exe is copied somewhere
# disposable: the real target directory must not collect crash files, and a
# stale one there would make the wizard suite's "nothing crashed" check lie.
$copy = Join-Path $work 'pacenotes-installer.exe'
Copy-Item -LiteralPath $ExePath -Destination $copy
try {
    # ---- a report where we point it ----
    $report = Join-Path $work 'run.log'
    Set-Content -Path $report -Value 'downloading something' -Encoding utf8
    $proc = Start-Process $copy -PassThru -Wait -ArgumentList @('--selftest-panic', '--out', $report)
    Assert-That 'the process dies as Windows reports a Rust abort' `
    ($proc.ExitCode -eq -1073740791) "exit 0x$(($proc.ExitCode -band 0xFFFFFFFF).ToString('X8'))"

    $text = Get-Content $report -Raw
    Assert-That 'the report was appended to the run log' ($text -match 'panicked at') "log said [$text]"
    Assert-That 'the log it was appended to is intact' ($text -match 'downloading something') `
        "log said [$text]"
    # The point of the whole exercise: a line number in OUR source.
    Assert-That 'it names the source file and line' ($text -match 'src\\[\w\\]+\.rs:\d+') `
        "log said [$text]"
    Assert-That 'it carries the panic message' ($text -match 'deliberate panic') "log said [$text]"

    # ---- and a report when nobody pointed it anywhere ----
    # A panic before the game folder is settled has no run log to go in.
    $beside = Join-Path $work 'pacenotes-installer-crash.log'
    Remove-Item $beside -ErrorAction SilentlyContinue
    $proc = Start-Process $copy -PassThru -Wait -ArgumentList @('--selftest-panic')
    Assert-That 'a panic with no log still leaves one beside the exe' (Test-Path $beside)
    if (Test-Path $beside) {
        Assert-That 'and it says the same thing' `
        ((Get-Content $beside -Raw) -match 'panicked at .*src\\[\w\\]+\.rs:\d+')
    }
} finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}

Write-Host "`n=== $($script:checks - $script:failures.Count)/$($script:checks) checks passed ===" -ForegroundColor Cyan
if ($script:failures.Count -gt 0) {
    Write-Host 'FAILED:' -ForegroundColor Red
    $script:failures | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    exit 1
}
Write-Host 'A panic leaves a usable report.' -ForegroundColor Green
exit 0
