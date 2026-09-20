<#
.SYNOPSIS
    AUTOMATED TEST RUNNER. Runs every installer check in order and exits
    non-zero if any of them fails.

.DESCRIPTION
    One entry point for the whole suite:

      1. cargo fmt --check   — the installer is excluded from the workspace,
                               so the root CI job's fmt/clippy never sees it
      2. cargo clippy        — ditto, under -D warnings as CI uses
      3. cargo test          — pure logic, cross-platform, no desktop needed
      4. Test-VersionSync    — Cargo.toml version vs the .rc VERSIONINFO
      5. Test-BinarySize.ps1 — the 500 kB release gate (builds the release exe
                               the next two steps run)
      6. Test-HttpClient.ps1 — the WinHTTP client against a live
                               pacenotes-server, including the auth matrix
      7. Test-UiLayout.ps1   — the real .exe driven through user32.dll,
                               once per shipped language

    Step 7 needs Windows with an interactive desktop session, since it shows
    real windows. Use -SkipUi on a headless machine; everything before it still
    runs — step 6 included, because --selftest-http writes to a file and never
    opens a window.

.PARAMETER SkipUi
    Skip the GUI tests (use on a headless agent or in a container).

.EXAMPLE
    pwsh -File tests/Run-AllTests.ps1
    pwsh -File tests/Run-AllTests.ps1 -SkipUi
#>
[CmdletBinding()]
param([switch]$SkipUi)

$ErrorActionPreference = 'Stop'
$crateRoot = Split-Path -Parent $PSScriptRoot
$results = New-Object System.Collections.ArrayList

function Invoke-Step {
    param([string]$Name, [scriptblock]$Body)
    Write-Host "`n########## $Name ##########" -ForegroundColor Cyan
    & $Body
    $ok = ($LASTEXITCODE -eq 0)
    [void]$results.Add([pscustomobject]@{ Name = $Name; Ok = $ok })
    if (-not $ok) { Write-Host "$Name FAILED (exit $LASTEXITCODE)" -ForegroundColor Red }
}

Push-Location $crateRoot
$savedRustflags = $env:RUSTFLAGS
try {
    # Mirror what CI denies, so a local run fails for the same reasons. Scoped
    # and restored: Test-HttpClient builds the SERVER, a different crate with
    # its own lint policy, and a stray RUSTFLAGS also forces a full rebuild of
    # its dependency tree every time the flags change.
    $env:RUSTFLAGS = '-D warnings'
    Invoke-Step 'cargo fmt --check' { cargo fmt --check }
    Invoke-Step 'cargo clippy' {
        cargo clippy --locked --all-targets --target x86_64-pc-windows-msvc
    }
    Invoke-Step 'cargo test (pure logic)' { cargo test --locked }
    Invoke-Step 'version sync' { pwsh -NoProfile -File (Join-Path $PSScriptRoot 'Test-VersionSync.ps1') }
    Invoke-Step 'binary size gate' { pwsh -NoProfile -File (Join-Path $PSScriptRoot 'Test-BinarySize.ps1') }
    Invoke-Step 'http client vs live server' { pwsh -NoProfile -File (Join-Path $PSScriptRoot 'Test-HttpClient.ps1') }

    if ($SkipUi) {
        Write-Host "`nSkipping GUI tests (-SkipUi)." -ForegroundColor Yellow
    } else {
        foreach ($lang in 0, 1) {
            $label = if ($lang -eq 0) { 'English' } else { 'Polish' }
            Invoke-Step "UI layout ($label)" {
                pwsh -NoProfile -File (Join-Path $PSScriptRoot 'ui\Test-UiLayout.ps1') -Language $lang
            }
        }
    }
} finally {
    $env:RUSTFLAGS = $savedRustflags
    Pop-Location
}

Write-Host "`n########## summary ##########" -ForegroundColor Cyan
foreach ($r in $results) {
    $tag = if ($r.Ok) { 'PASS' } else { 'FAIL' }
    $color = if ($r.Ok) { 'Green' } else { 'Red' }
    Write-Host ("  {0}  {1}" -f $tag, $r.Name) -ForegroundColor $color
}

$failed = @($results | Where-Object { -not $_.Ok })
if ($failed.Count -gt 0) { exit 1 }
Write-Host 'All installer checks passed.' -ForegroundColor Green
exit 0
