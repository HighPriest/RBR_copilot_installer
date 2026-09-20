<#
.SYNOPSIS
    AUTOMATED TEST. Drives the real installer .exe and asserts its dialog
    behaves as installer/DESIGN.md specifies. Exits 0 on success, 1 on any
    failed assertion.

.DESCRIPTION
    Catches what `cargo build` cannot: a caption wired to the wrong string id,
    a translation that was never added, a tab page that fails to hide, state
    that does not survive a game switch, a folder field that accepts a folder
    it should reject.

    Two rules this test follows deliberately:

    1. Expected text is READ FROM res/installer.rc, not duplicated here.
       Hardcoding the strings would only prove two copies match, and would
       still pass if a control were bound to the wrong id. Reading the
       resource asserts the real property: this control shows what the
       resource defines for this language.

    2. Inputs are fixtures that stand for a REAL user situation, never
       arbitrary paths or placeholder rows. The game-folder field exists to
       tell "this is your Richard Burns Rally install" from "this is some
       other folder", so the fixtures are exactly that: a directory containing
       a game's marker executable, and one that is not.

    Windows-only, and needs an interactive desktop session (it shows real
    windows). The cross-platform half of the suite is `cargo test`.

.PARAMETER Language
    0 = English (default), 1 = Polish. The Polish pass proves the per-language
    STRINGTABLE lookup and that UTF-8 diacritics survive into real controls.

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
$IDC_LBL_I_VOICE = 221; $IDC_LBL_I_UILANG = 223; $IDC_LBL_I_STYLE = 225
$IDC_I_VOICE = 222; $IDC_I_UILANG = 224; $IDC_I_STYLE = 226; $IDC_INSTALL = 227
$IDC_RIGHTTAB = 230
$IDC_LBL_KEY = 240; $IDC_UNLOCK = 242; $IDC_LBL_D_VOICE = 243
$IDC_D_VOICE = 244; $IDC_DOWNLOAD = 249
$IDC_LBL_BACKUPS = 260; $IDC_BACKUPS = 261; $IDC_RESTORE = 262
$IDC_STATUS = 271
$IDC_INFOTAB = 280; $IDC_NOTES = 281; $IDC_LOGS = 282
$IDOK = 1; $IDCANCEL = 2

# Ribbon order and marker executables mirror src/gamedir.rs, which the
# cross-platform `cargo test --test gamedir` pins to the legacy C# names.
$GAME_TAB_RBR = 0
$GAME_TAB_BEAMNG = 2
$RBR_TITLE = 'Richard Burns Rally'
$RBR_MARKER = 'RichardBurnsRally_SSE.exe'
$BEAMNG_MARKER = 'BeamNG.drive.exe'

$S = Get-ResourceStrings -Language $(if ($Language -eq 0) { 'en' } else { 'pl' })

# The language picker is shown BEFORE a language has been chosen, so it renders
# in whatever the OS UI language resolves to — it cannot honor a choice the
# user has not made yet. Asserting it against -Language would be asserting the
# wrong thing. What must hold is that its captions are real shipped strings for
# their ids, not the "<id>" fallback i18n::load_string emits for a missing or
# mis-bound id. So we accept any language the binary ships.
$EN_STRINGS = Get-ResourceStrings -Language 'en'
$PL_STRINGS = Get-ResourceStrings -Language 'pl'

function Assert-AnyShipped {
    param([string]$Name, [AllowNull()][string]$Actual, [string]$Key)
    $allowed = @($EN_STRINGS[$Key], $PL_STRINGS[$Key])
    Assert-That -Name $Name -Condition ($allowed -ccontains $Actual) `
        -Detail "expected one of [$($allowed -join '] [')], got [$Actual]"
}

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

function Assert-Equal {
    param([string]$Name, [AllowNull()][string]$Actual, [string]$Expected)
    Assert-That -Name $Name -Condition ($Actual -ceq $Expected) `
        -Detail "expected [$Expected], got [$Actual]"
}

<#  Build a directory that IS a given game's install folder, by placing the
    marker executable the installer identifies that game by. #>
function New-GameFolder {
    param([Parameter(Mandatory)][string]$Marker)
    $dir = Join-Path ([System.IO.Path]::GetTempPath()) ("pacenotes-uitest-" + [guid]::NewGuid())
    [void](New-Item -ItemType Directory -Path $dir)
    # Contents are irrelevant: the installer identifies a game by the presence
    # of this executable, exactly as the legacy C# did.
    [void](New-Item -ItemType File -Path (Join-Path $dir $Marker))
    return $dir
}

Write-Host "=== Installer UI test (language index $Language) ===" -ForegroundColor Cyan

$rbrFolder = New-GameFolder -Marker $RBR_MARKER
$launch = Start-Installer -ExePath $ExePath
$proc = $launch.Process
Write-Host "exe:        $($launch.Path)"
Write-Host "RBR folder: $rbrFolder"

try {
    # ---------------------------------------------------------------
    # Language picker
    # ---------------------------------------------------------------
    Write-Host "`n-- language picker --"
    $picker = Wait-ForDialog -OwnerPid $proc.Id
    Assert-That 'language picker appears' ($null -ne $picker)
    if (-not $picker) { throw 'no picker; cannot continue' }

    Assert-AnyShipped 'picker title is a shipped string' $picker.Title 'IDS_LANG_TITLE'
    Assert-AnyShipped 'picker prompt is a shipped string' `
    (Get-CtrlText $picker.Handle $IDC_LANGPROMPT) 'IDS_LANG_PROMPT'
    Assert-AnyShipped 'picker restart note is a shipped string (DESIGN.md)' `
    (Get-CtrlText $picker.Handle $IDC_LANGRESTART) 'IDS_LANG_RESTART'

    # One entry per language the binary actually ships; both must be offered
    # or a user cannot reach the other STRINGTABLE at all.
    $combo = [NativeUi]::GetDlgItem($picker.Handle, $IDC_LANGCOMBO)
    $count = [int][NativeUi]::SendMessageW($combo, 0x0146, [IntPtr]::Zero, [IntPtr]::Zero)
    Assert-That 'picker offers English and Polski' ($count -eq 2) "got $count entries"

    [void][NativeUi]::SendMessageW($combo, 0x014E, [IntPtr]$Language, [IntPtr]::Zero)
    Invoke-Button -Dialog $picker.Handle -CtrlId $IDOK

    # ---------------------------------------------------------------
    # Main dialog: every caption must match the resource for this language
    # ---------------------------------------------------------------
    Write-Host "`n-- localized captions --"
    $main = Wait-ForDialog -OwnerPid $proc.Id -NotHandle "$($picker.Handle)"
    Assert-That 'main dialog appears' ($null -ne $main)
    if (-not $main) { throw 'no main dialog; cannot continue' }
    $dlg = $main.Handle

    Assert-Equal 'window title' $main.Title $S['IDS_APP_TITLE']
    foreach ($c in @(
            @{ Id = $IDC_LBL_GAMEDIR; Key = 'IDS_LBL_GAMEDIR'; What = 'game folder label' },
            @{ Id = $IDC_BROWSE; Key = 'IDS_BTN_BROWSE'; What = 'browse button' },
            @{ Id = $IDC_LBL_INSTALLED; Key = 'IDS_LBL_INSTALLED'; What = 'downloaded-content heading' },
            @{ Id = $IDC_LBL_I_VOICE; Key = 'IDS_CAT_VOICE'; What = 'installed voice label' },
            @{ Id = $IDC_LBL_I_UILANG; Key = 'IDS_CAT_UILANG'; What = 'installed ui-language label' },
            @{ Id = $IDC_LBL_I_STYLE; Key = 'IDS_CAT_STYLE'; What = 'installed callouts label' },
            @{ Id = $IDC_INSTALL; Key = 'IDS_BTN_INSTALL'; What = 'install button' },
            @{ Id = $IDC_LBL_KEY; Key = 'IDS_LBL_ACCESSKEY'; What = 'access key label' },
            @{ Id = $IDC_UNLOCK; Key = 'IDS_BTN_UNLOCK'; What = 'unlock button' },
            @{ Id = $IDC_LBL_D_VOICE; Key = 'IDS_CAT_VOICE'; What = 'downloadable voice label' },
            @{ Id = $IDC_DOWNLOAD; Key = 'IDS_BTN_DOWNLOAD'; What = 'download button' },
            @{ Id = $IDC_LBL_BACKUPS; Key = 'IDS_LBL_BACKUPS'; What = 'backups heading' },
            @{ Id = $IDC_RESTORE; Key = 'IDS_BTN_RESTORE'; What = 'restore button' },
            @{ Id = $IDCANCEL; Key = 'IDS_BTN_CLOSE'; What = 'close button' },
            @{ Id = $IDC_NOTES; Key = 'IDS_NOTES_BODY'; What = 'notes guidance text' },
            @{ Id = $IDC_STATUS; Key = 'IDS_READY'; What = 'initial status line' }
        )) {
        Assert-Equal $c.What (Get-CtrlText $dlg $c.Id) $S[$c.Key]
    }

    # ---------------------------------------------------------------
    # Tabs: one per game, and the two page pairs DESIGN.md calls for
    # ---------------------------------------------------------------
    Write-Host "`n-- tab structure --"
    Assert-That 'ribbon has one tab per supported game' `
    ((Get-TabCount -Dialog $dlg -TabCtrlId $IDC_GAMETAB) -eq 3) `
        "got $(Get-TabCount -Dialog $dlg -TabCtrlId $IDC_GAMETAB)"
    Assert-That 'right column offers Download and Backups' `
    ((Get-TabCount -Dialog $dlg -TabCtrlId $IDC_RIGHTTAB) -eq 2)
    Assert-That 'bottom offers Notes and Logs' `
    ((Get-TabCount -Dialog $dlg -TabCtrlId $IDC_INFOTAB) -eq 2)

    Assert-That 'Download is the default right page (DESIGN.md)' (Test-CtrlVisible $dlg $IDC_D_VOICE)
    Assert-That 'Backups page hidden until selected' (-not (Test-CtrlVisible $dlg $IDC_BACKUPS))
    Assert-That 'Notes is the default bottom page' (Test-CtrlVisible $dlg $IDC_NOTES)
    Assert-That 'Logs pane hidden until needed' (-not (Test-CtrlVisible $dlg $IDC_LOGS))

    Write-Host "`n-- right column tab switching --"
    $sel = Select-Tab -Dialog $dlg -TabCtrlId $IDC_RIGHTTAB -Index 1
    Assert-That 'Backups tab selects' ($sel -eq 1) "selection is $sel"
    Assert-That 'Backups page shown' (Test-CtrlVisible $dlg $IDC_BACKUPS)
    Assert-That 'Download page hidden' (-not (Test-CtrlVisible $dlg $IDC_D_VOICE))
    $sel = Select-Tab -Dialog $dlg -TabCtrlId $IDC_RIGHTTAB -Index 0
    Assert-That 'Download tab reselects' ($sel -eq 0)
    Assert-That 'Download page shown again' (Test-CtrlVisible $dlg $IDC_D_VOICE)

    # ---------------------------------------------------------------
    # Game folder: the field must identify the GAME, not just a directory
    # ---------------------------------------------------------------
    Write-Host "`n-- game folder validation --"
    Assert-Equal 'blank field asks for a folder' (Get-CtrlText $dlg $IDC_DIRSTATUS) $S['IDS_DIR_EMPTY']

    $missing = Join-Path $rbrFolder 'no-such-subfolder'
    Set-CtrlText -Dialog $dlg -CtrlId $IDC_GAMEDIR -Value $missing
    Invoke-EditChanged -Dialog $dlg -CtrlId $IDC_GAMEDIR
    Assert-Equal 'mistyped path reported as missing' (Get-CtrlText $dlg $IDC_DIRSTATUS) $S['IDS_DIR_NOTFOUND']

    # A real directory that is not a game install must NOT be accepted.
    $notAGame = Join-Path ([System.IO.Path]::GetTempPath()) ("pacenotes-uitest-plain-" + [guid]::NewGuid())
    [void](New-Item -ItemType Directory -Path $notAGame)
    try {
        Set-CtrlText -Dialog $dlg -CtrlId $IDC_GAMEDIR -Value $notAGame
        Invoke-EditChanged -Dialog $dlg -CtrlId $IDC_GAMEDIR
        Assert-Equal 'ordinary folder rejected, naming the missing exe' `
        (Get-CtrlText $dlg $IDC_DIRSTATUS) (Expand-ResourceArg $S['IDS_DIR_NOTGAME'] $RBR_MARKER)
    } finally {
        Remove-Item -Recurse -Force $notAGame -ErrorAction SilentlyContinue
    }

    # The genuine article, on the tab that matches it.
    Set-CtrlText -Dialog $dlg -CtrlId $IDC_GAMEDIR -Value $rbrFolder
    Invoke-EditChanged -Dialog $dlg -CtrlId $IDC_GAMEDIR
    Assert-Equal 'RBR folder accepted on the RBR tab, naming the exe found' `
    (Get-CtrlText $dlg $IDC_DIRSTATUS) (Expand-ResourceArg $S['IDS_DIR_OK'] $RBR_MARKER)

    # Same folder, wrong game: validation is per-game or the ribbon is a lie.
    Write-Host "`n-- validation is per game --"
    $sel = Select-Tab -Dialog $dlg -TabCtrlId $IDC_GAMETAB -Index $GAME_TAB_BEAMNG
    Assert-That 'switched to the BeamNG tab' ($sel -eq $GAME_TAB_BEAMNG) "selection is $sel"
    Set-CtrlText -Dialog $dlg -CtrlId $IDC_GAMEDIR -Value $rbrFolder
    Invoke-EditChanged -Dialog $dlg -CtrlId $IDC_GAMEDIR
    Assert-Equal 'an RBR folder is refused for BeamNG, naming BeamNG.drive.exe' `
    (Get-CtrlText $dlg $IDC_DIRSTATUS) (Expand-ResourceArg $S['IDS_DIR_NOTGAME'] $BEAMNG_MARKER)

    # ---------------------------------------------------------------
    # Install preconditions and error routing
    # ---------------------------------------------------------------
    Write-Host "`n-- install preconditions --"
    # BeamNG tab currently holds a folder that is not a BeamNG install.
    Invoke-Button -Dialog $dlg -CtrlId $IDC_INSTALL
    Start-Sleep -Milliseconds 250
    Assert-Equal 'install refused while the folder is not this game' `
    (Get-CtrlText $dlg $IDC_STATUS) $S['IDS_ERR_NO_DIR']
    Assert-That 'error switched the bottom area to Logs (DESIGN.md)' `
    ((Get-TabSelection -Dialog $dlg -TabCtrlId $IDC_INFOTAB) -eq 1)
    Assert-That 'Logs pane shown' (Test-CtrlVisible $dlg $IDC_LOGS)
    Assert-That 'Notes pane hidden behind Logs' (-not (Test-CtrlVisible $dlg $IDC_NOTES))

    # Back on a valid RBR folder the folder check passes, so the NEXT
    # precondition — a pick in each category — is what refuses.
    $sel = Select-Tab -Dialog $dlg -TabCtrlId $IDC_GAMETAB -Index $GAME_TAB_RBR
    Assert-That 'switched back to the RBR tab' ($sel -eq $GAME_TAB_RBR)
    Invoke-Button -Dialog $dlg -CtrlId $IDC_INSTALL
    Start-Sleep -Milliseconds 250
    Assert-Equal 'with a valid folder, install refuses on missing picks instead' `
    (Get-CtrlText $dlg $IDC_STATUS) $S['IDS_ERR_NO_PICK']

    # The log is the user's record of what went wrong, per game.
    $log = Get-CtrlText $dlg $IDC_LOGS
    Assert-That 'log opens with the startup entry' `
    ($log.StartsWith("[$RBR_TITLE] " + $S['IDS_LOG_STARTED'])) "log begins [$log]"
    # .Contains, not -like: the game prefix is bracketed, and PowerShell's
    # -like would read "[BeamNG]" as a wildcard character class matching a
    # single letter, so the assertion would pass on almost any log text.
    Assert-That 'log attributes the folder error to the game it happened on' `
    ($log.Contains("[BeamNG] " + $S['IDS_ERR_NO_DIR'])) "log is [$log]"
    Assert-That 'log records the missing-picks error against RBR' `
    ($log.Contains("[$RBR_TITLE] " + $S['IDS_ERR_NO_PICK'])) "log is [$log]"

    # NOTE: the "all three categories picked -> install proceeds" path is NOT
    # asserted here. The category lists are filled from the server manifest,
    # which task I5 adds; injecting placeholder rows would assert nothing about
    # real behavior. That case belongs with the code that populates them.

    # ---------------------------------------------------------------
    # Per-game state (DESIGN.md: identical layout, per-game content)
    # ---------------------------------------------------------------
    Write-Host "`n-- per-game state --"
    Assert-Equal 'RBR tab kept its own folder' (Get-CtrlText $dlg $IDC_GAMEDIR) $rbrFolder
    $sel = Select-Tab -Dialog $dlg -TabCtrlId $IDC_GAMETAB -Index $GAME_TAB_BEAMNG
    Assert-That 'switched to BeamNG again' ($sel -eq $GAME_TAB_BEAMNG)
    Assert-Equal 'BeamNG tab kept the folder typed into it, not RBR''s' `
    (Get-CtrlText $dlg $IDC_GAMEDIR) $rbrFolder
    Assert-Equal 'and still judges it against BeamNG' `
    (Get-CtrlText $dlg $IDC_DIRSTATUS) (Expand-ResourceArg $S['IDS_DIR_NOTGAME'] $BEAMNG_MARKER)

    # ---------------------------------------------------------------
    # Shutdown
    # ---------------------------------------------------------------
    Write-Host "`n-- shutdown --"
    Close-Dialog -Dialog $dlg
    Assert-That 'WM_CLOSE exits the process cleanly' ($proc.WaitForExit(4000))
} finally {
    if (-not $proc.HasExited) { $proc.Kill(); $proc.WaitForExit(2000) | Out-Null }
    Remove-Item -Recurse -Force $rbrFolder -ErrorAction SilentlyContinue
}

Write-Host "`n=== $($script:checks - $script:failures.Count)/$($script:checks) checks passed ===" -ForegroundColor Cyan
if ($script:failures.Count -gt 0) {
    Write-Host 'FAILED:' -ForegroundColor Red
    $script:failures | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    exit 1
}
Write-Host 'All UI checks passed.' -ForegroundColor Green
exit 0
