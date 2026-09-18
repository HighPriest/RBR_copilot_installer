# Release gate: fail if the installer exceeds 500 kB.
# Usage: .\check-size.ps1   (builds release, then checks)
$ErrorActionPreference = "Stop"
$LimitKB = 500

cargo build --release --target x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = "target\x86_64-pc-windows-msvc\release\pacenotes-installer.exe"
if (-not (Test-Path $exe)) { $exe = "..\target\x86_64-pc-windows-msvc\release\pacenotes-installer.exe" }

$sizeKB = [math]::Round((Get-Item $exe).Length / 1KB, 1)
Write-Host "Installer size: $sizeKB kB (limit: $LimitKB kB)"

if ($sizeKB -gt $LimitKB) {
    Write-Error "SIZE GATE FAILED: $sizeKB kB > $LimitKB kB. See installer/CLAUDE.md size audit workflow."
    exit 1
}
Write-Host "Size gate OK." -ForegroundColor Green
