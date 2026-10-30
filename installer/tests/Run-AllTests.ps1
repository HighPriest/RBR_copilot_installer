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
      7. Test-UiLayout.ps1   — scenes 1-3 against a dead server (the
                               offline path), once per shipped language
      8. Test-WizardDownload — scenes 1-4 against a LIVE server holding a
                               nested catalogue

    Steps 7 and 8 need Windows with an interactive desktop session, since they
    show
    real windows. Use -SkipUi on a headless machine; everything before them still
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
    # Denies what CI denies; Test-HttpClient clears it again for the server crate.
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
        Invoke-Step 'wizard download scene' {
            pwsh -NoProfile -File (Join-Path $PSScriptRoot 'ui\Test-WizardDownload.ps1')
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
