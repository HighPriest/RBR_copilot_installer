<#
.SYNOPSIS
    AUTOMATED TEST RUNNER. Runs every installer check in order and exits
    non-zero if any of them fails.

.DESCRIPTION
    One entry point for the whole suite:

      1. cargo test          — pure logic, cross-platform, no desktop needed
      2. Test-BinarySize.ps1 — the 500 kB release gate
      3. Test-UiLayout.ps1   — the real .exe driven through user32.dll,
                               once per shipped language

    Step 3 needs Windows with an interactive desktop session, since it shows
    real windows. Use -SkipUi on a headless machine; the first two steps still
    give you the cross-platform logic and the size gate.

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
try {
    Invoke-Step 'cargo test (pure logic)' { cargo test --locked }
    Invoke-Step 'binary size gate' { pwsh -NoProfile -File (Join-Path $PSScriptRoot 'Test-BinarySize.ps1') }

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
