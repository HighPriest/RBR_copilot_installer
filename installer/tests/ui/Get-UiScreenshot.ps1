<#
.SYNOPSIS
    AGENT / DEVELOPER AID. Not a test — captures a PNG of each wizard scene so
    a human or an AI agent can SEE the layout.

.DESCRIPTION
    Test-UiLayout.ps1 proves a control exists, is enabled and holds the right
    text. It cannot tell you a label is clipped, two controls overlap, or a
    page has an ugly dead area. This closes that gap: run it, then open (or,
    as an agent, `Read`) the images.

    A tab-header Z-order bug once survived a fully green assertion run and was
    only caught here, so use this whenever you change geometry or add a scene.

    Asserts nothing and always exits 0. Never gate CI on it.

.PARAMETER Language
    0 = English (default), 1 = Polish. Polish strings are longer and are the
    usual cause of clipped labels, so check both when changing a layout.

.PARAMETER Scenes
    How many scenes to walk and capture, from 1. Default 5 (all that exist).

.PARAMETER Server
    Which server to point the wizard at. The default is a dead address, which
    keeps scenes 1-3 identical every run — but it leaves scenes 4 and 5 empty,
    since there is no catalogue to show. Pass a real server to capture those
    two with content; the live public endpoint is the useful one, because its
    co-driver paths are the longest text this layout has to hold.

.PARAMETER GameFolder
    Folder to type into scene 3. Defaults to a throwaway folder containing a
    Richard Burns Rally marker, so the dropdown shows a detected game rather
    than the placeholder.

.PARAMETER OutDir
    Where the PNGs land. Defaults to the system temp folder, deliberately NOT
    the repo: these are throwaway artifacts.

.EXAMPLE
    pwsh -File Get-UiScreenshot.ps1
    pwsh -File Get-UiScreenshot.ps1 -Language 1
#>
[CmdletBinding()]
param(
    [ValidateSet(0, 1)][int]$Language = 0,
    [ValidateRange(1, 5)][int]$Scenes = 5,
    [string]$GameFolder,
    [string]$OutDir = $env:TEMP,
    [string]$ExePath,
    [string]$Server = 'http://127.0.0.1:1'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'Win32Ui.psm1') -Force
Add-Type -AssemblyName System.Drawing

$IDC_S1_COMBO = 302
$IDC_S2_MOTD = 311
$IDC_S3_PATH = 322
$IDC_S4_TREE = 331
$IDC_S5_LANG = 342
$PSBTN_NEXT = 1

function Save-WindowPng {
    <#  PrintWindow, not CopyFromScreen.

        Windows refuses SetForegroundWindow to a process that does not hold
        the foreground, so a screen grab silently captures whatever happens to
        be on top — which produced screenshots of unrelated applications here.
        PrintWindow asks the window to render itself into our DC and does not
        care about z-order. PW_RENDERFULLCONTENT (2) is needed for the themed
        common controls to draw. #>
    param([Parameter(Mandatory)][IntPtr]$Window, [Parameter(Mandatory)][string]$Path)
    [void][NativeUi]::SetForegroundWindow($Window)   # nice when allowed
    Start-Sleep -Milliseconds 400
    $r = New-Object RECT
    [void][NativeUi]::GetWindowRect($Window, [ref]$r)
    $w = $r.R - $r.L; $h = $r.B - $r.T
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    $ok = [NativeUi]::PrintWindow($Window, $hdc, 2)
    $g.ReleaseHdc($hdc)
    if (-not $ok) {
        # A screen grab can capture occluding windows, hence the warning.
        Write-Warning 'PrintWindow failed; falling back to a screen grab'
        $g.CopyFromScreen($r.L, $r.T, 0, 0, (New-Object System.Drawing.Size $w, $h))
    }
    $g.Dispose()
    $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Host ("saved {0}  ({1}x{2})" -f $Path, $w, $h)
}

$temp = $null
if (-not $GameFolder) {
    $temp = Join-Path ([System.IO.Path]::GetTempPath()) ("pacenotes-shot-" + [guid]::NewGuid())
    [void](New-Item -ItemType Directory -Path $temp)
    [void](New-Item -ItemType File -Path (Join-Path $temp 'RichardBurnsRally_SSE.exe'))
    $GameFolder = $temp
}

# A dead server by default keeps scene 2 on its offline path; -Server gives scenes 4 and 5 real content.
$launch = Start-Installer -ExePath $ExePath -ExtraArgs @('--server', $Server)
$proc = $launch.Process

try {
    $sheet = Wait-ForDialog -OwnerPid $proc.Id
    if (-not $sheet) { throw 'wizard never appeared' }
    $sheetH = $sheet.Handle

    $page = Wait-ForPageControl -Sheet $sheetH -CtrlId $IDC_S1_COMBO
    if ($page -eq [IntPtr]::Zero) { throw 'scene 1 never appeared' }
    $combo = [NativeUi]::GetDlgItem($page, $IDC_S1_COMBO)
    [void][NativeUi]::SendMessageW($combo, 0x014E, [IntPtr]$Language, [IntPtr]::Zero)
    Save-WindowPng -Window $sheetH -Path (Join-Path $OutDir "wizard-s1-lang$Language.png")
    if ($Scenes -lt 2) { return }

    Press-WizardButton -Sheet $sheetH -Button $PSBTN_NEXT
    $page = Wait-ForPageControl -Sheet $sheetH -CtrlId $IDC_S2_MOTD
    if ($page -eq [IntPtr]::Zero) { throw 'scene 2 never appeared' }
    Start-Sleep -Seconds 2   # let the server-status line settle
    Save-WindowPng -Window $sheetH -Path (Join-Path $OutDir "wizard-s2-lang$Language.png")
    if ($Scenes -lt 3) { return }

    Press-WizardButton -Sheet $sheetH -Button $PSBTN_NEXT
    $page = Wait-ForPageControl -Sheet $sheetH -CtrlId $IDC_S3_PATH
    if ($page -eq [IntPtr]::Zero) { throw 'scene 3 never appeared' }
    # Fill the folder so the shot shows a detected game, not the placeholder.
    Set-CtrlText -Dialog $page -CtrlId $IDC_S3_PATH -Value $GameFolder
    Invoke-EditChanged -Dialog $page -CtrlId $IDC_S3_PATH
    Start-Sleep -Milliseconds 900
    Save-WindowPng -Window $sheetH -Path (Join-Path $OutDir "wizard-s3-lang$Language.png")
    if ($Scenes -lt 4) { return }

    Press-WizardButton -Sheet $sheetH -Button $PSBTN_NEXT
    $page = Wait-ForPageControl -Sheet $sheetH -CtrlId $IDC_S4_TREE
    if ($page -eq [IntPtr]::Zero) { throw 'scene 4 never appeared' }
    # One opened category shows the nesting once the catalogue has arrived.
    Start-Sleep -Seconds 3
    Save-WindowPng -Window $sheetH -Path (Join-Path $OutDir "wizard-s4-lang$Language.png")
    if ($Scenes -lt 5) { return }

    # Scene 5 lists what scene 4 committed, so tick everything first.
    [void](Invoke-TreeCheckRoots -Dialog $page -CtrlId $IDC_S4_TREE)
    Press-WizardButton -Sheet $sheetH -Button $PSBTN_NEXT
    $page = Wait-ForPageControl -Sheet $sheetH -CtrlId $IDC_S5_LANG
    if ($page -eq [IntPtr]::Zero) { throw 'scene 5 never appeared' }
    Start-Sleep -Milliseconds 500
    Save-WindowPng -Window $sheetH -Path (Join-Path $OutDir "wizard-s5-lang$Language.png")
} finally {
    if (-not $proc.HasExited) { $proc.Kill(); $proc.WaitForExit(2000) | Out-Null }
    if ($temp) { Remove-Item -Recurse -Force $temp -ErrorAction SilentlyContinue }
}
