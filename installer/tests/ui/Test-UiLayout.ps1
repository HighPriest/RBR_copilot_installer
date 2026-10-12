<#
.SYNOPSIS
    AUTOMATED TEST. Walks the installer's wizard from scene 1 to scene 3 and
    asserts it behaves as installer/DESIGN.md rev. 2 specifies. Exits 0 on
    success, 1 on any failed assertion.

.DESCRIPTION
    Catches what `cargo build` cannot: a caption bound to the wrong string id,
    a translation never added, a page that does not appear, the game dropdown
    not reflecting what is on disk, Next enabled when it should not be.

    Two rules this test follows deliberately (see tests/README.md):

    1. Expected text is READ FROM res/installer.rc, not duplicated here, so a
       caption wired to the wrong id is caught rather than mirrored.
    2. Inputs are fixtures that stand for a REAL user situation — folders
       containing real game marker executables, not arbitrary paths.

    Runs against a deliberately dead server (`--server http://127.0.0.1:1`) so
    scene 2 takes its documented OFFLINE path and the run is deterministic.
    The online paths are covered by tests/Test-HttpClient.ps1.

    Windows-only, and needs an interactive desktop session.

.PARAMETER Language
    0 = English (default), 1 = Polish. Scene 1 picks this, so scenes 2 and 3
    are asserted against that language's STRINGTABLE — which is what proves
    the per-language lookup and the UTF-8 diacritics.

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

# ---- Control ids: must match res/installer.rc and src/wizard.rs ----
$IDC_SCENE_TITLE = 299
$IDC_S1_PROMPT = 301; $IDC_S1_COMBO = 302; $IDC_S1_NOTE = 303
$IDC_S2_MOTD = 311; $IDC_S2_SECRET_LABEL = 312; $IDC_S2_SECRET = 313; $IDC_S2_STATUS = 314
$IDC_S3_PATH_LABEL = 321; $IDC_S3_PATH = 322; $IDC_S3_BROWSE = 323
$IDC_S3_GAME_LABEL = 324; $IDC_S3_GAME = 325

$ID_WIZBACK = 0x3023; $ID_WIZNEXT = 0x3024
$PSBTN_NEXT = 1; $PSBTN_BACK = 0

# Markers mirror src/gamedir.rs, which `cargo test --test gamedir` pins to the
# ids and names the protocol and the legacy installer use.
$RBR_MARKER = 'RichardBurnsRally_SSE.exe'
$AC_MARKER = 'assettocorsa.exe'
$RBR_TITLE = 'Richard Burns Rally'

$S = Get-ResourceStrings -Language $(if ($Language -eq 0) { 'en' } else { 'pl' })
$EN_STRINGS = Get-ResourceStrings -Language 'en'
$PL_STRINGS = Get-ResourceStrings -Language 'pl'

$script:failures = New-Object System.Collections.ArrayList
$script:checks = 0

function Assert-That {
    param([Parameter(Mandatory)][string]$Name, [Parameter(Mandatory)][bool]$Condition, [string]$Detail = '')
    $script:checks++
    if ($Condition) {
        Write-Host "  PASS  $Name" -ForegroundColor Green
    } else {
        Write-Host ("  FAIL  {0}{1}" -f $Name, $(if ($Detail) { " -- $Detail" } else { '' })) -ForegroundColor Red
        [void]$script:failures.Add($Name)
    }
}

function Assert-Equal {
    param([string]$Name, [AllowNull()][string]$Actual, [string]$Expected)
    Assert-That -Name $Name -Condition ($Actual -ceq $Expected) -Detail "expected [$Expected], got [$Actual]"
}

<#  Scene 1 is shown BEFORE a language is chosen, so it renders in the OS UI
    language and cannot honor -Language. What must hold is that its captions
    are real shipped strings, not the "<id>" fallback a mis-bound id gives. #>
function Assert-AnyShipped {
    param([string]$Name, [AllowNull()][string]$Actual, [string]$Key)
    $allowed = @($EN_STRINGS[$Key], $PL_STRINGS[$Key])
    Assert-That -Name $Name -Condition ($allowed -ccontains $Actual) `
        -Detail "expected one of [$($allowed -join '] [')], got [$Actual]"
}

<#  WM_GETTEXT on a window rather than a control of a dialog. Same reason
    Get-CtrlText uses it: GetWindowText does not cross process boundaries. #>
function Get-WindowTextOf {
    param([Parameter(Mandatory)][IntPtr]$Window)
    $cap = 512
    $sb = New-Object System.Text.StringBuilder $cap
    [void][NativeUi]::SendMessageGetText($Window, 0x000D, [IntPtr]$cap, $sb)
    return $sb.ToString()
}

<#  A directory that IS an install of the given games, identified the way the
    installer identifies one: by the marker executables it contains. #>
function New-GameFolder {
    param([string[]]$Markers)
    $dir = Join-Path ([System.IO.Path]::GetTempPath()) ("pacenotes-wiz-" + [guid]::NewGuid())
    [void](New-Item -ItemType Directory -Path $dir)
    foreach ($m in $Markers) { [void](New-Item -ItemType File -Path (Join-Path $dir $m)) }
    return $dir
}

<#  Put a path in the folder field and let the debounced scan settle. #>
function Set-FolderAndSettle {
    param([Parameter(Mandatory)][IntPtr]$Page, [Parameter(Mandatory)][string]$Path)
    Set-CtrlText -Dialog $Page -CtrlId $IDC_S3_PATH -Value $Path
    Invoke-EditChanged -Dialog $Page -CtrlId $IDC_S3_PATH
    Start-Sleep -Milliseconds 900   # debounce is 400 ms in wizard.rs
}

Write-Host "=== Installer wizard test (language index $Language) ===" -ForegroundColor Cyan

$rbrFolder = New-GameFolder -Markers @($RBR_MARKER)
$twoGames = New-GameFolder -Markers @($RBR_MARKER, $AC_MARKER)
$plainFolder = New-GameFolder -Markers @()

$launch = Start-Installer -ExePath $ExePath -ExtraArgs @('--server', 'http://127.0.0.1:1')
$proc = $launch.Process
Write-Host "exe:        $($launch.Path)"
Write-Host "RBR folder: $rbrFolder"

try {
    # ---------------------------------------------------------------
    # Scene 1 — Language
    # ---------------------------------------------------------------
    Write-Host "`n-- scene 1: language --"
    $sheet = Wait-ForDialog -OwnerPid $proc.Id
    Assert-That 'wizard window appears' ($null -ne $sheet)
    if (-not $sheet) { throw 'no wizard; cannot continue' }
    $sheetH = $sheet.Handle

    $page = Wait-ForPageControl -Sheet $sheetH -CtrlId $IDC_S1_COMBO
    Assert-That 'scene 1 is the first page shown' ($page -ne [IntPtr]::Zero)
    if ($page -eq [IntPtr]::Zero) { throw 'scene 1 never appeared' }

    Assert-AnyShipped 'window caption is a shipped string' $sheet.Title 'IDS_APP_TITLE'
    Assert-AnyShipped 'scene 1 prompt' (Get-CtrlText $page $IDC_S1_PROMPT) 'IDS_S1_PROMPT'
    Assert-AnyShipped 'scene 1 restart note' (Get-CtrlText $page $IDC_S1_NOTE) 'IDS_S1_RESTART'
    Assert-AnyShipped 'primary button is labelled for this scene' `
    (Get-WizardButtonText -Sheet $sheetH -ButtonId $ID_WIZNEXT) 'IDS_BTN_CONTINUE'

    $combo = [NativeUi]::GetDlgItem($page, $IDC_S1_COMBO)
    $count = [int][NativeUi]::SendMessageW($combo, 0x0146, [IntPtr]::Zero, [IntPtr]::Zero)
    Assert-That 'both shipped languages are offered' ($count -eq 2) "got $count"
    # DESIGN.md: Back exists only on scenes 3-5.
    Assert-That 'Back is unavailable on the first scene' `
    (-not (Test-WizardButtonEnabled -Sheet $sheetH -ButtonId $ID_WIZBACK))

    # Choose the language under test, then continue.
    [void][NativeUi]::SendMessageW($combo, 0x014E, [IntPtr]$Language, [IntPtr]::Zero)
    Press-WizardButton -Sheet $sheetH -Button $PSBTN_NEXT

    # ---------------------------------------------------------------
    # Scene 2 — Message of the day (offline path)
    # ---------------------------------------------------------------
    Write-Host "`n-- scene 2: message of the day --"
    $page = Wait-ForPageControl -Sheet $sheetH -CtrlId $IDC_S2_MOTD
    Assert-That 'scene 2 follows scene 1' ($page -ne [IntPtr]::Zero)
    if ($page -eq [IntPtr]::Zero) { throw 'scene 2 never appeared' }

    # From here on the chosen language must be in force — including the title
    # bar, which each scene re-sets.
    Assert-Equal 'window caption follows the chosen language' `
    (Get-WindowTextOf $sheetH) $S['IDS_APP_TITLE']
    Assert-Equal 'scene 2 title' (Get-CtrlText $page $IDC_SCENE_TITLE) $S['IDS_S2_TITLE']
    Assert-Equal 'secret label' (Get-CtrlText $page $IDC_S2_SECRET_LABEL) $S['IDS_S2_SECRET']
    # comctl32 supplies Back and Cancel in the OS language; DESIGN.md counts
    # them as scene chrome, so they must follow the chosen language too.
    Assert-Equal 'Cancel button is localized' `
    (Get-WizardButtonText -Sheet $sheetH -ButtonId 2) $S['IDS_BTN_CANCEL']
    Assert-Equal 'primary button reads ACCEPT' `
    (Get-WizardButtonText -Sheet $sheetH -ButtonId $ID_WIZNEXT) $S['IDS_BTN_ACCEPT']
    Assert-That 'Back still unavailable on scene 2 (DESIGN.md)' `
    (-not (Test-WizardButtonEnabled -Sheet $sheetH -ButtonId $ID_WIZBACK))

    # The server is dead, so the offline copy must be what is shown and the
    # status line must settle on Offline rather than stay on "Checking".
    Assert-Equal 'offline notice is shown in the MOTD pane' `
    (Get-CtrlText $page $IDC_S2_MOTD) $S['IDS_S2_OFFLINE_MOTD']
    $deadline = (Get-Date).AddSeconds(15)
    do {
        Start-Sleep -Milliseconds 300
        $status = Get-CtrlText $page $IDC_S2_STATUS
    } while ($status -cne $S['IDS_S2_OFFLINE'] -and (Get-Date) -lt $deadline)
    Assert-Equal 'server status settles on Offline' $status $S['IDS_S2_OFFLINE']

    # Empty secret continues anonymously (DESIGN.md).
    Press-WizardButton -Sheet $sheetH -Button $PSBTN_NEXT

    # ---------------------------------------------------------------
    # Scene 3 — Game folder
    # ---------------------------------------------------------------
    Write-Host "`n-- scene 3: game folder --"
    $page = Wait-ForPageControl -Sheet $sheetH -CtrlId $IDC_S3_PATH
    Assert-That 'scene 3 follows scene 2' ($page -ne [IntPtr]::Zero)
    if ($page -eq [IntPtr]::Zero) { throw 'scene 3 never appeared' }

    Assert-Equal 'scene 3 title' (Get-CtrlText $page $IDC_SCENE_TITLE) $S['IDS_S3_TITLE']
    Assert-Equal 'folder label' (Get-CtrlText $page $IDC_S3_PATH_LABEL) $S['IDS_S3_PATH']
    Assert-Equal 'game label' (Get-CtrlText $page $IDC_S3_GAME_LABEL) $S['IDS_S3_GAME']
    Assert-Equal 'browse button' (Get-CtrlText $page $IDC_S3_BROWSE) $S['IDS_BTN_BROWSE']
    Assert-Equal 'primary button reads Next' `
    (Get-WizardButtonText -Sheet $sheetH -ButtonId $ID_WIZNEXT) $S['IDS_BTN_NEXT']
    Assert-Equal 'Back button is localized' `
    (Get-WizardButtonText -Sheet $sheetH -ButtonId $ID_WIZBACK) $S['IDS_BTN_BACK']
    Assert-That 'Back becomes available on scene 3 (DESIGN.md)' `
    (Test-WizardButtonEnabled -Sheet $sheetH -ButtonId $ID_WIZBACK)

    # Nothing entered yet: placeholder, disabled dropdown, no way forward.
    $gameCombo = [NativeUi]::GetDlgItem($page, $IDC_S3_GAME)
    Assert-Equal 'dropdown starts on the detecting placeholder' `
    (Get-CtrlText $page $IDC_S3_GAME) $S['IDS_S3_DETECTING']
    Assert-That 'dropdown is disabled while nothing is detected' `
    (-not [NativeUi]::IsWindowEnabled($gameCombo))
    Assert-That 'Next is blocked until a game is detected' `
    (-not (Test-WizardButtonEnabled -Sheet $sheetH -ButtonId $ID_WIZNEXT))

    # A real folder that is not a game must not unlock anything.
    Set-FolderAndSettle -Page $page -Path $plainFolder
    Assert-Equal 'an ordinary folder leaves the placeholder' `
    (Get-CtrlText $page $IDC_S3_GAME) $S['IDS_S3_DETECTING']
    Assert-That 'an ordinary folder leaves Next blocked' `
    (-not (Test-WizardButtonEnabled -Sheet $sheetH -ButtonId $ID_WIZNEXT))

    # One marker: that game, shown but not selectable, and Next opens up.
    Set-FolderAndSettle -Page $page -Path $rbrFolder
    Assert-Equal 'a single game is named in the dropdown' (Get-CtrlText $page $IDC_S3_GAME) $RBR_TITLE
    Assert-That 'dropdown stays disabled when there is nothing to choose' `
    (-not [NativeUi]::IsWindowEnabled($gameCombo))
    Assert-That 'Next unlocks once a game is detected' `
    (Test-WizardButtonEnabled -Sheet $sheetH -ButtonId $ID_WIZNEXT)

    # Two markers: the dropdown becomes a real choice listing only the matches.
    Set-FolderAndSettle -Page $page -Path $twoGames
    $items = [int][NativeUi]::SendMessageW($gameCombo, 0x0146, [IntPtr]::Zero, [IntPtr]::Zero)
    Assert-That 'both detected games are offered' ($items -eq 2) "dropdown holds $items"
    Assert-That 'dropdown is selectable when the folder holds several games' `
    ([NativeUi]::IsWindowEnabled($gameCombo))
    Assert-Equal 'the first match is preselected' (Get-CtrlText $page $IDC_S3_GAME) $RBR_TITLE
    Assert-That 'Next stays available' (Test-WizardButtonEnabled -Sheet $sheetH -ButtonId $ID_WIZNEXT)

    # Going back to nothing detected must re-lock the scene.
    Set-FolderAndSettle -Page $page -Path $plainFolder
    Assert-Equal 'clearing the detection restores the placeholder' `
    (Get-CtrlText $page $IDC_S3_GAME) $S['IDS_S3_DETECTING']
    Assert-That 'and blocks Next again' `
    (-not (Test-WizardButtonEnabled -Sheet $sheetH -ButtonId $ID_WIZNEXT))

    # ---------------------------------------------------------------
    # Back navigation
    # ---------------------------------------------------------------
    Write-Host "`n-- back navigation --"
    Press-WizardButton -Sheet $sheetH -Button $PSBTN_BACK
    $back = Wait-ForPageControl -Sheet $sheetH -CtrlId $IDC_S2_MOTD -TimeoutMs 3000
    Assert-That 'Back returns to scene 2' ($back -ne [IntPtr]::Zero)
    Assert-Equal 'scene 2 primary button is restored' `
    (Get-WizardButtonText -Sheet $sheetH -ButtonId $ID_WIZNEXT) $S['IDS_BTN_ACCEPT']
} finally {
    if (-not $proc.HasExited) { $proc.Kill(); $proc.WaitForExit(2000) | Out-Null }
    foreach ($d in @($rbrFolder, $twoGames, $plainFolder)) {
        Remove-Item -Recurse -Force $d -ErrorAction SilentlyContinue
    }
}

Write-Host "`n=== $($script:checks - $script:failures.Count)/$($script:checks) checks passed ===" -ForegroundColor Cyan
if ($script:failures.Count -gt 0) {
    Write-Host 'FAILED:' -ForegroundColor Red
    $script:failures | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    exit 1
}
Write-Host 'All wizard checks passed.' -ForegroundColor Green
exit 0
