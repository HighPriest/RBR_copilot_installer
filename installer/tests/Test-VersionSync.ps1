<#
.SYNOPSIS
    AUTOMATED TEST. The version in installer/Cargo.toml and the VERSIONINFO in
    res/installer.rc must agree. Exits non-zero if they drift.

.DESCRIPTION
    The version lives in four places: `[package] version` in Cargo.toml, and
    FILEVERSION, PRODUCTVERSION and the FileVersion/ProductVersion strings in
    res/installer.rc. Until now a comment in the .rc was the only thing holding
    them together.

    That matters because the CD workflows read Cargo.toml to decide the release
    tag and to refuse a tag that already exists. If the .rc lags behind, a
    release tagged v0.2.0 ships an .exe whose properties still say 0.1.0 —
    exactly the kind of mismatch nobody notices until a user reports the wrong
    version in the file properties dialog.

    Cargo.toml is treated as the source of truth, since that is what the
    workflows tag from. A Cargo semver X.Y.Z must appear in the .rc as
    "X,Y,Z,0" for the binary fields and "X.Y.Z.0" for the string fields.

.EXAMPLE
    pwsh -File tests/Test-VersionSync.ps1
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$crateRoot = Split-Path -Parent $PSScriptRoot
$cargoPath = Join-Path $crateRoot 'Cargo.toml'
$rcPath = Join-Path $crateRoot 'res\installer.rc'

$failures = New-Object System.Collections.ArrayList
function Assert-That {
    param([string]$Name, [bool]$Condition, [string]$Detail = '')
    if ($Condition) {
        Write-Host "  PASS  $Name" -ForegroundColor Green
    } else {
        Write-Host ("  FAIL  {0}{1}" -f $Name, $(if ($Detail) { " -- $Detail" } else { '' })) -ForegroundColor Red
        [void]$failures.Add($Name)
    }
}

Write-Host '=== version sync (Cargo.toml <-> installer.rc) ===' -ForegroundColor Cyan

# Anchored at line start so a dependency's `version = "=..."` never matches; the CD workflows tag with the same expression.
$cargoText = Get-Content -Raw $cargoPath
$cargoMatch = [regex]::Match($cargoText, '(?m)^version\s*=\s*"(?<v>[^"]+)"')
if (-not $cargoMatch.Success) { Write-Error "No [package] version in $cargoPath"; exit 1 }
$cargoVersion = $cargoMatch.Groups['v'].Value
Write-Host "Cargo.toml version: $cargoVersion"

$semver = [regex]::Match($cargoVersion, '^(?<x>\d+)\.(?<y>\d+)\.(?<z>\d+)$')
Assert-That 'Cargo version is a plain X.Y.Z' $semver.Success "got '$cargoVersion'"
if (-not $semver.Success) { exit 1 }

$expectedCommas = '{0},{1},{2},0' -f $semver.Groups['x'].Value, $semver.Groups['y'].Value, $semver.Groups['z'].Value
$expectedDots = '{0}.{1}.{2}.0' -f $semver.Groups['x'].Value, $semver.Groups['y'].Value, $semver.Groups['z'].Value
Write-Host "expected in .rc:    $expectedCommas / $expectedDots"

$rcText = Get-Content -Raw -Encoding UTF8 $rcPath

foreach ($field in 'FILEVERSION', 'PRODUCTVERSION') {
    # \b so FILEVERSION does not also match inside PRODUCTVERSION.
    $m = [regex]::Match($rcText, "(?m)^\s*$field\s+(?<v>[\d\s,]+?)\s*$")
    Assert-That "$field present in installer.rc" $m.Success
    if ($m.Success) {
        $actual = $m.Groups['v'].Value -replace '\s', ''
        Assert-That "$field matches Cargo.toml" ($actual -eq $expectedCommas) `
            "expected [$expectedCommas], got [$actual]"
    }
}

foreach ($field in 'FileVersion', 'ProductVersion') {
    $m = [regex]::Match($rcText, "VALUE\s+`"$field`"\s*,\s*`"(?<v>[^`"]+)`"")
    Assert-That "VALUE `"$field`" present in installer.rc" $m.Success
    if ($m.Success) {
        Assert-That "VALUE `"$field`" matches Cargo.toml" ($m.Groups['v'].Value -eq $expectedDots) `
            "expected [$expectedDots], got [$($m.Groups['v'].Value)]"
    }
}

if ($failures.Count -gt 0) {
    Write-Host "`nFAILED:" -ForegroundColor Red
    $failures | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    Write-Host 'Update res/installer.rc to match installer/Cargo.toml (or vice versa).' -ForegroundColor Red
    exit 1
}
Write-Host 'Version fields are in sync.' -ForegroundColor Green
exit 0
