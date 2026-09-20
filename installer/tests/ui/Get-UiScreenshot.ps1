<#
.SYNOPSIS
    AGENT / DEVELOPER AID. Not a test — captures PNG screenshots of the
    installer's dialogs so a human or an AI agent can SEE the layout.

.DESCRIPTION
    Test-UiLayout.ps1 proves controls exist, are visible and hold the right
    text, but it cannot tell you that a label is clipped, two buttons overlap,
    or a panel has an ugly dead area. This script exists to close that gap:
    run it, then open (or, for an agent, read) the PNGs.

    It asserts nothing and always exits 0. Do not wire it into CI as a gate.

.PARAMETER Language
    0 = English (default), 1 = Polish. Polish strings are longer and are the
    usual cause of clipped labels, so check both when changing the layout.

.PARAMETER RightTab
    0 = Download page (default), 1 = Backups page.

.PARAMETER InfoTab
    0 = Notes pane (default), 1 = Logs pane.

.PARAMETER GameTab
    Index into the game ribbon: 0 = Richard Burns Rally, 1 = Dirt Rally,
    2 = BeamNG.

.PARAMETER OutDir
    Where the PNGs land. Defaults to the system temp folder, deliberately NOT
    the repo: these are throwaway artifacts.

.EXAMPLE
    # Look at the default English layout
    pwsh -File Get-UiScreenshot.ps1

.EXAMPLE
    # Polish, Backups page, Logs pane — the tightest label case
    pwsh -File Get-UiScreenshot.ps1 -Language 1 -RightTab 1 -InfoTab 1
#>
[CmdletBinding()]
param(
    [ValidateSet(0, 1)][int]$Language = 0,
    [ValidateSet(0, 1)][int]$RightTab = 0,
    [ValidateSet(0, 1)][int]$InfoTab = 0,
    [ValidateRange(0, 2)][int]$GameTab = 0,
    [string]$OutDir = $env:TEMP,
    [string]$ExePath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'Win32Ui.psm1') -Force
Add-Type -AssemblyName System.Drawing

$IDC_LANGCOMBO = 101
$IDC_GAMETAB = 200
$IDC_RIGHTTAB = 230
$IDC_INFOTAB = 280
$IDOK = 1

function Save-WindowPng {
    param([Parameter(Mandatory)][IntPtr]$Window, [Parameter(Mandatory)][string]$Path)
    [void][NativeUi]::SetForegroundWindow($Window)
    Start-Sleep -Milliseconds 400
    $r = New-Object RECT
    [void][NativeUi]::GetWindowRect($Window, [ref]$r)
    $w = $r.R - $r.L; $h = $r.B - $r.T
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.L, $r.T, 0, 0, (New-Object System.Drawing.Size $w, $h))
    $g.Dispose()
    $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Host ("saved {0}  ({1}x{2})" -f $Path, $w, $h)
}

$launch = Start-Installer -ExePath $ExePath
$proc = $launch.Process
$suffix = "lang$Language-game$GameTab-right$RightTab-info$InfoTab"

try {
    $picker = Wait-ForDialog -OwnerPid $proc.Id
    if (-not $picker) { throw 'language picker never appeared' }
    Save-WindowPng -Window $picker.Handle -Path (Join-Path $OutDir "installer-picker-lang$Language.png")

    $combo = [NativeUi]::GetDlgItem($picker.Handle, $IDC_LANGCOMBO)
    [void][NativeUi]::SendMessageW($combo, 0x014E, [IntPtr]$Language, [IntPtr]::Zero)
    Invoke-Button -Dialog $picker.Handle -CtrlId $IDOK

    $main = Wait-ForDialog -OwnerPid $proc.Id -NotHandle "$($picker.Handle)"
    if (-not $main) { throw 'main dialog never appeared' }

    if ($GameTab -ne 0) { [void](Select-Tab -Dialog $main.Handle -TabCtrlId $IDC_GAMETAB -Index $GameTab) }
    if ($RightTab -ne 0) { [void](Select-Tab -Dialog $main.Handle -TabCtrlId $IDC_RIGHTTAB -Index $RightTab) }
    if ($InfoTab -ne 0) { [void](Select-Tab -Dialog $main.Handle -TabCtrlId $IDC_INFOTAB -Index $InfoTab) }

    Save-WindowPng -Window $main.Handle -Path (Join-Path $OutDir "installer-main-$suffix.png")
} finally {
    if (-not $proc.HasExited) { $proc.Kill(); $proc.WaitForExit(2000) | Out-Null }
}
