<#
.SYNOPSIS
    AUTOMATED TEST. Asserts the installer's dialog layout and UI behavior
    against installer/DESIGN.md. Exits 0 on success, 1 on any failed assertion.

.DESCRIPTION
    Runs the real release .exe and drives it through user32.dll, so it catches
    things a `cargo build` cannot: a control missing from the dialog template,
    a caption that never got localized, a tab page that does not hide, a
    validation message that does not update, per-game state that is not kept.

    Windows-only, and it needs a desktop session (it shows real windows).
    CI runs it from the installer job; see installer/tests/README.md.

.PARAMETER Language
    0 = English (default), 1 = Polish. The Polish pass proves the STRINGTABLE
    per-language lookup and the UTF-8 diacritics survive into real controls.

.PARAMETER ExePath
    Override the binary under test. Defaults to the release build.

.EXAMPLE
    pwsh -File Test-UiLayout.ps1
    pwsh -File Test-UiLayout.ps1 -Language 1
#>
[CmdletBinding()]
param(
    [ValidateSet(0, 1)][int]$Language = 0,
    [string]$ExePath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'Win32Ui.psm1') -Force

# ---- Control ids: must match res/installer.rc and src/ui.rs ----
$IDC_LANGCOMBO = 101; $IDC_LANGPROMPT = 102; $IDC_LANGRESTART = 103
$IDC_GAMETAB = 200
$IDC_LBL_GAMEDIR = 210; $IDC_GAMEDIR = 211; $IDC_BROWSE = 212; $IDC_DIRSTATUS = 213
$IDC_LBL_INSTALLED = 220
$IDC_I_VOICE = 222; $IDC_I_UILANG = 224; $IDC_I_STYLE = 226; $IDC_INSTALL = 227
$IDC_RIGHTTAB = 230
$IDC_KEY = 241; $IDC_UNLOCK = 242; $IDC_D_VOICE = 244; $IDC_DOWNLOAD = 249
$IDC_BACKUPS = 261; $IDC_RESTORE = 262
$IDC_PROGRESS = 270; $IDC_STATUS = 271
$IDC_INFOTAB = 280; $IDC_NOTES = 281; $IDC_LOGS = 282
$IDOK = 1; $IDCANCEL = 2

$GAMES = @('Richard Burns Rally', 'Dirt Rally', 'BeamNG')

# Expected localized fragments, keyed by the -Language index.
$EXPECT = @(
    @{ Title = 'Pacenotes Installer'; Install = 'Install'; Close = 'Close'
        DirEmpty = 'Enter or browse'; DirBad = 'does not exist'; DirOk = 'Folder found'
        NoPick = 'every category'; Voice = 'Co-driver voice' },
    @{ Title = 'Instalator Pacenotes'; Install = 'Instaluj'; Close = 'Zakończ'
        DirEmpty = 'Wpisz lub wskaż'; DirBad = 'nie istnieje'; DirOk = 'Folder znaleziony'
        NoPick = 'każdej kategorii'; Voice = 'Głos pilota rajdowego' }
)[$Language]

$script:failures = New-Object System.Collections.ArrayList
$script:checks = 0

function Assert-That {
    param([Parameter(Mandatory)][string]$Name, [Parameter(Mandatory)][bool]$Condition, [string]$Detail = '')
    $script:checks++
    if ($Condition) {
        Write-Host ("  PASS  {0}" -f $Name) -ForegroundColor Green
    } else {
        Write-Host ("  FAIL  {0}{1}" -f $Name, $(if ($Detail) { " -- $Detail" } else { '' })) -ForegroundColor Red
        [void]$script:failures.Add($Name)
    }
}

function Assert-Contains {
    param([string]$Name, [AllowNull()][string]$Haystack, [string]$Needle)
    Assert-That -Name $Name -Condition ($null -ne $Haystack -and $Haystack -like "*$Needle*") `
        -Detail "expected to contain '$Needle', got '$Haystack'"
}

Write-Host "=== Installer UI layout test (language index $Language) ===" -ForegroundColor Cyan

$launch = Start-Installer -ExePath $ExePath
$proc = $launch.Process
Write-Host "exe: $($launch.Path)"

try {
    # ---------------------------------------------------------------
    # Language picker
    # ---------------------------------------------------------------
    Write-Host "`n-- language picker --"
    $picker = Wait-ForDialog -OwnerPid $proc.Id
    Assert-That 'language picker appears' ($null -ne $picker)
    if (-not $picker) { throw 'no picker; cannot continue' }

    $combo = [NativeUi]::GetDlgItem($picker.Handle, $IDC_LANGCOMBO)
    $count = [int][NativeUi]::SendMessageW($combo, 0x0146, [IntPtr]::Zero, [IntPtr]::Zero)
    Assert-That 'picker offers 2 languages' ($count -eq 2) "got $count"
    Assert-That 'picker has a prompt' (-not [string]::IsNullOrWhiteSpace((Get-CtrlText $picker.Handle $IDC_LANGPROMPT)))
    Assert-That 'picker shows the restart note (DESIGN.md)' `
    (-not [string]::IsNullOrWhiteSpace((Get-CtrlText $picker.Handle $IDC_LANGRESTART)))

    # Choose the requested language, then OK.
    [void][NativeUi]::SendMessageW($combo, 0x014E, [IntPtr]$Language, [IntPtr]::Zero)
    Invoke-Button -Dialog $picker.Handle -CtrlId $IDOK

    # ---------------------------------------------------------------
    # Main dialog
    # ---------------------------------------------------------------
    Write-Host "`n-- main dialog --"
    $main = Wait-ForDialog -OwnerPid $proc.Id -NotHandle "$($picker.Handle)"
    Assert-That 'main dialog appears' ($null -ne $main)
    if (-not $main) { throw 'no main dialog; cannot continue' }
    $dlg = $main.Handle
    Assert-Contains 'window title is localized' $main.Title $EXPECT.Title

    $inv = Get-ControlInventory -Dialog $dlg
    foreach ($req in @(
            @{ Id = $IDC_GAMETAB; What = 'game ribbon' },
            @{ Id = $IDC_GAMEDIR; What = 'game folder edit' },
            @{ Id = $IDC_BROWSE; What = 'browse button' },
            @{ Id = $IDC_DIRSTATUS; What = 'folder validation label' },
            @{ Id = $IDC_I_VOICE; What = 'installed voice list' },
            @{ Id = $IDC_I_UILANG; What = 'installed ui-language list' },
            @{ Id = $IDC_I_STYLE; What = 'installed callouts-style list' },
            @{ Id = $IDC_INSTALL; What = 'install button' },
            @{ Id = $IDC_RIGHTTAB; What = 'download/backups tabs' },
            @{ Id = $IDC_KEY; What = 'private key edit' },
            @{ Id = $IDC_D_VOICE; What = 'downloadable voice list' },
            @{ Id = $IDC_BACKUPS; What = 'backups list' },
            @{ Id = $IDC_PROGRESS; What = 'status progress bar' },
            @{ Id = $IDC_STATUS; What = 'status text' },
            @{ Id = $IDC_INFOTAB; What = 'notes/logs tabs' },
            @{ Id = $IDC_NOTES; What = 'notes pane' },
            @{ Id = $IDC_LOGS; What = 'logs pane' }
        )) {
        Assert-That "control present: $($req.What)" `
        (@($inv | Where-Object { $_.Id -eq $req.Id }).Count -eq 1)
    }

    # ---------------------------------------------------------------
    # Tab structure (DESIGN.md: game ribbon; Download/Backups; Notes/Logs)
    # ---------------------------------------------------------------
    Write-Host "`n-- tab structure --"
    Assert-That 'game ribbon has one tab per game' `
    ((Get-TabCount -Dialog $dlg -TabCtrlId $IDC_GAMETAB) -eq $GAMES.Count)
    Assert-That 'right column has 2 tabs' ((Get-TabCount -Dialog $dlg -TabCtrlId $IDC_RIGHTTAB) -eq 2)
    Assert-That 'bottom has 2 tabs' ((Get-TabCount -Dialog $dlg -TabCtrlId $IDC_INFOTAB) -eq 2)

    Assert-That 'Download is the default right page' (Test-CtrlVisible $dlg $IDC_D_VOICE)
    Assert-That 'Backups page starts hidden' (-not (Test-CtrlVisible $dlg $IDC_BACKUPS))
    Assert-That 'Notes is the default bottom page' (Test-CtrlVisible $dlg $IDC_NOTES)
    Assert-That 'Logs pane starts hidden' (-not (Test-CtrlVisible $dlg $IDC_LOGS))
    Assert-That 'notes pane carries guidance text' `
    ((Get-CtrlText $dlg $IDC_NOTES).Length -gt 40)

    # ---------------------------------------------------------------
    # Localization reached real controls
    # ---------------------------------------------------------------
    Write-Host "`n-- localization --"
    Assert-Contains 'install button localized' (Get-CtrlText $dlg $IDC_INSTALL) $EXPECT.Install
    Assert-Contains 'close button localized' (Get-CtrlText $dlg $IDCANCEL) $EXPECT.Close
    Assert-Contains 'voice category label localized' (Get-CtrlText $dlg 221) $EXPECT.Voice

    # ---------------------------------------------------------------
    # Right-hand tab switching
    # ---------------------------------------------------------------
    Write-Host "`n-- right column tab switching --"
    $sel = Select-Tab -Dialog $dlg -TabCtrlId $IDC_RIGHTTAB -Index 1
    Assert-That 'selected the Backups tab' ($sel -eq 1) "selection is $sel"
    Assert-That 'Backups page now visible' (Test-CtrlVisible $dlg $IDC_BACKUPS)
    Assert-That 'Download page now hidden' (-not (Test-CtrlVisible $dlg $IDC_D_VOICE))
    $sel = Select-Tab -Dialog $dlg -TabCtrlId $IDC_RIGHTTAB -Index 0
    Assert-That 'back to Download tab' ($sel -eq 0)
    Assert-That 'Download page visible again' (Test-CtrlVisible $dlg $IDC_D_VOICE)

    # ---------------------------------------------------------------
    # Game folder validation (DESIGN.md: "the field should be validated")
    # ---------------------------------------------------------------
    Write-Host "`n-- game folder validation --"
    Assert-Contains 'empty folder prompts the user' (Get-CtrlText $dlg $IDC_DIRSTATUS) $EXPECT.DirEmpty

    Set-CtrlText -Dialog $dlg -CtrlId $IDC_GAMEDIR -Value 'Z:\definitely\not\here'
    Invoke-EditChanged -Dialog $dlg -CtrlId $IDC_GAMEDIR
    Assert-Contains 'missing folder is rejected' (Get-CtrlText $dlg $IDC_DIRSTATUS) $EXPECT.DirBad

    $realDir = $env:TEMP
    Set-CtrlText -Dialog $dlg -CtrlId $IDC_GAMEDIR -Value $realDir
    Invoke-EditChanged -Dialog $dlg -CtrlId $IDC_GAMEDIR
    Assert-Contains 'existing folder is accepted' (Get-CtrlText $dlg $IDC_DIRSTATUS) $EXPECT.DirOk

    # ---------------------------------------------------------------
    # Install preconditions + error routing to the Logs tab
    # ---------------------------------------------------------------
    Write-Host "`n-- install preconditions / error routing --"
    Invoke-Button -Dialog $dlg -CtrlId $IDC_INSTALL
    Start-Sleep -Milliseconds 250
    Assert-Contains 'install without picks is refused' (Get-CtrlText $dlg $IDC_STATUS) $EXPECT.NoPick
    Assert-That 'error auto-switched to the Logs tab (DESIGN.md)' `
    ((Get-TabSelection -Dialog $dlg -TabCtrlId $IDC_INFOTAB) -eq 1)
    Assert-That 'logs pane became visible' (Test-CtrlVisible $dlg $IDC_LOGS)
    Assert-That 'notes pane hidden while logs shown' (-not (Test-CtrlVisible $dlg $IDC_NOTES))
    Assert-Contains 'log records the active game' (Get-CtrlText $dlg $IDC_LOGS) $GAMES[0]
    Assert-Contains 'log records the error' (Get-CtrlText $dlg $IDC_LOGS) $EXPECT.NoPick

    # With one pick in every category the precondition passes instead.
    Add-ListItem -Dialog $dlg -CtrlId $IDC_I_VOICE
    Add-ListItem -Dialog $dlg -CtrlId $IDC_I_UILANG
    Add-ListItem -Dialog $dlg -CtrlId $IDC_I_STYLE
    Invoke-Button -Dialog $dlg -CtrlId $IDC_INSTALL
    Start-Sleep -Milliseconds 250
    Assert-That 'install with all categories picked clears the refusal' `
    ((Get-CtrlText $dlg $IDC_STATUS) -notlike "*$($EXPECT.NoPick)*")

    # ---------------------------------------------------------------
    # Per-game state (DESIGN.md: identical layout, per-game content)
    # ---------------------------------------------------------------
    Write-Host "`n-- per-game state --"
    $sel = Select-Tab -Dialog $dlg -TabCtrlId $IDC_GAMETAB -Index 1
    Assert-That 'switched to the second game tab' ($sel -eq 1) "selection is $sel"
    Assert-That 'second game starts with its own empty folder' `
    ([string]::IsNullOrWhiteSpace((Get-CtrlText $dlg $IDC_GAMEDIR))) `
        "got '$(Get-CtrlText $dlg $IDC_GAMEDIR)'"

    Set-CtrlText -Dialog $dlg -CtrlId $IDC_GAMEDIR -Value 'C:\Windows'
    Invoke-EditChanged -Dialog $dlg -CtrlId $IDC_GAMEDIR
    $sel = Select-Tab -Dialog $dlg -TabCtrlId $IDC_GAMETAB -Index 0
    Assert-That 'switched back to the first game tab' ($sel -eq 0)
    Assert-That 'first game folder was preserved' `
    ((Get-CtrlText $dlg $IDC_GAMEDIR) -eq $realDir) "got '$(Get-CtrlText $dlg $IDC_GAMEDIR)'"
    $sel = Select-Tab -Dialog $dlg -TabCtrlId $IDC_GAMETAB -Index 1
    Assert-That 'second game folder was preserved' `
    ((Get-CtrlText $dlg $IDC_GAMEDIR) -eq 'C:\Windows') "got '$(Get-CtrlText $dlg $IDC_GAMEDIR)'"

    # ---------------------------------------------------------------
    # Shutdown
    # ---------------------------------------------------------------
    Write-Host "`n-- shutdown --"
    Close-Dialog -Dialog $dlg
    $exited = $proc.WaitForExit(4000)
    Assert-That 'WM_CLOSE exits the process cleanly' $exited
} finally {
    if (-not $proc.HasExited) { $proc.Kill(); $proc.WaitForExit(2000) | Out-Null }
}

Write-Host "`n=== $($script:checks - $script:failures.Count)/$($script:checks) checks passed ===" -ForegroundColor Cyan
if ($script:failures.Count -gt 0) {
    Write-Host "FAILED:" -ForegroundColor Red
    $script:failures | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    exit 1
}
Write-Host "All UI layout checks passed." -ForegroundColor Green
exit 0
