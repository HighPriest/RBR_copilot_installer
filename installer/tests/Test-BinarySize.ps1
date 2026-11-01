<#
.SYNOPSIS
    AUTOMATED TEST. The 700 kB release gate, and that the installer needs nothing
    Windows does not ship. Exits non-zero if the binary is over budget or
    imports any other DLL.

.DESCRIPTION
    The whole architecture of this crate — WinHTTP instead of reqwest, raw
    dialogs instead of a GUI framework, a hand-rolled zip reader instead of the
    `zip` crate — exists to hold this number down. See installer/CLAUDE.md.

    Builds release for x86_64-pc-windows-msvc, then measures, then reads the
    exe's import table: a dependency such as VCRUNTIME140.dll (the Visual C++
    redistributable) would stop the installer before it can say a word on a
    PC without it, so it is held out the same way the size is.

.PARAMETER LimitKB
    Budget in kB. Defaults to the 700 kB hard constraint from the root
    CLAUDE.md; only pass something else to explore headroom locally.

.EXAMPLE
    pwsh -File tests/Test-BinarySize.ps1
#>
[CmdletBinding()]
param([int]$LimitKB = 700)

$ErrorActionPreference = 'Stop'

<#  The DLL names in a PE32+ file's import table, read straight from the file
    so the gate needs no Visual Studio tools to be found. #>
function Get-ImportedDll {
    param([Parameter(Mandatory)][string]$Path)
    $pe = [System.IO.File]::ReadAllBytes((Resolve-Path $Path))
    $header = [BitConverter]::ToInt32($pe, 0x3C)
    $sections = [BitConverter]::ToUInt16($pe, $header + 6)
    $optional = $header + 24
    $sectionTable = $optional + [BitConverter]::ToUInt16($pe, $header + 20)
    # Data directory 1 (imports) of a PE32+ optional header.
    $importRva = [BitConverter]::ToUInt32($pe, $optional + 120)
    $toOffset = {
        param($rva)
        for ($i = 0; $i -lt $sections; $i++) {
            $entry = $sectionTable + 40 * $i
            $start = [BitConverter]::ToUInt32($pe, $entry + 12)
            $size = [BitConverter]::ToUInt32($pe, $entry + 8)
            if ($rva -ge $start -and $rva -lt $start + $size) {
                return $rva - $start + [BitConverter]::ToUInt32($pe, $entry + 20)
            }
        }
        throw "RVA $rva is in no section"
    }
    $names = @()
    for ($descriptor = & $toOffset $importRva; ; $descriptor += 20) {
        $nameRva = [BitConverter]::ToUInt32($pe, $descriptor + 12)
        if ($nameRva -eq 0) { break }
        $at = & $toOffset $nameRva
        $end = [Array]::IndexOf($pe, [byte]0, $at)
        $names += [System.Text.Encoding]::ASCII.GetString($pe, $at, $end - $at).ToLowerInvariant()
    }
    return $names
}

$crateRoot = Split-Path -Parent $PSScriptRoot
Push-Location $crateRoot
try {
    cargo build --release --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    # The crate is outside the workspace, so its own target comes first.
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

    # Windows' own DLLs, and the Universal CRT that ships with Windows 10 and later.
    $foreign = @(Get-ImportedDll $exe | Where-Object {
            $_ -notmatch '^(kernel32|user32|gdi32|shell32|comctl32|ntdll|ole32|advapi32|winhttp)\.dll$' -and
            $_ -notmatch '^api-ms-win-'
        })
    if ($foreign.Count -gt 0) {
        # Parenthesised, or -f formats only the second string and the names never appear.
        Write-Error (('IMPORT GATE FAILED: the installer needs {0}, which Windows does not ship. ' +
                'See installer/build/runtime_link.rs.') -f ($foreign -join ', '))
        exit 1
    }
    Write-Host 'Imports only what Windows ships.' -ForegroundColor Green
    exit 0
} finally {
    Pop-Location
}
