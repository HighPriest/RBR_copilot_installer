<#
.SYNOPSIS
    AUTOMATED TEST. The 500 kB release gate. Exits non-zero if the installer
    binary is over budget.

.DESCRIPTION
    The whole architecture of this crate — WinHTTP instead of reqwest, raw
    dialogs instead of a GUI framework, a hand-rolled zip reader instead of the
    `zip` crate — exists to hold this number down. See installer/CLAUDE.md.

    Builds release for x86_64-pc-windows-msvc, then measures.

.PARAMETER LimitKB
    Budget in kB. Defaults to the 500 kB hard constraint from the root
    CLAUDE.md; only pass something else to explore headroom locally.

.EXAMPLE
    pwsh -File tests/Test-BinarySize.ps1
#>
[CmdletBinding()]
param([int]$LimitKB = 500)

$ErrorActionPreference = 'Stop'

# Run from the crate root regardless of where the caller invoked us.
$crateRoot = Split-Path -Parent $PSScriptRoot
Push-Location $crateRoot
try {
    cargo build --release --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    # The crate is excluded from the workspace, so it normally has its own
    # target/. Fall back to a workspace-level one in case that ever changes.
    $exe = 'target\x86_64-pc-windows-msvc\release\pacenotes-installer.exe'
    if (-not (Test-Path $exe)) {
        $exe = '..\target\x86_64-pc-windows-msvc\release\pacenotes-installer.exe'
    }
    if (-not (Test-Path $exe)) {
        Write-Error "Built binary not found; looked for $exe"
        exit 1
    }

    $bytes = (Get-Item $exe).Length
    $sizeKB = [math]::Round($bytes / 1KB, 1)
    Write-Host "Installer size: $sizeKB kB ($bytes bytes), limit: $LimitKB kB"

    if ($sizeKB -gt $LimitKB) {
        Write-Error "SIZE GATE FAILED: $sizeKB kB > $LimitKB kB. See installer/CLAUDE.md size audit workflow."
        exit 1
    }
    Write-Host 'Size gate OK.' -ForegroundColor Green
    exit 0
} finally {
    Pop-Location
}
