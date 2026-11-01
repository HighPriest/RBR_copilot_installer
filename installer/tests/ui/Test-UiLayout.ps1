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

# Control ids, matching res/installer.rc and src/wizard/control_ids.rs.
$IDC_SCENE_TITLE = 299
$IDC_S1_PROMPT = 301; $IDC_S1_COMBO = 302; $IDC_S1_NOTE = 303
$IDC_S2_MOTD = 311; $IDC_S2_SECRET_LABEL = 312; $IDC_S2_SECRET = 313; $IDC_S2_STATUS = 314
$IDC_S3_PATH_LABEL = 321; $IDC_S3_PATH = 322; $IDC_S3_BROWSE = 323
$IDC_S3_GAME_LABEL = 324; $IDC_S3_GAME = 325
$IDC_S3_DOWNLOAD = 326; $IDC_S3_CONFIGURE = 327; $IDC_S3_ROUTES = 328; $IDC_S3_RESTORE = 329
$IDC_S4_TREE = 331
$VK_TAB = 0x09; $VK_RETURN = 0x0D

$ID_WIZBACK = 0x3023; $ID_WIZNEXT = 0x3024
$PSBTN_NEXT = 1; $PSBTN_BACK = 0

# Markers mirror src/gamedir.rs, whose unit tests pin them to the
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
    Start-Sleep -Milliseconds 900   # debounce is 400 ms in src/wizard/scene/game_folder.rs
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
    Write-Host "`n-- scene 1: language --"
    $sheet = Wait-ForDialog -OwnerPid $proc.Id
    Assert-That 'wizard window appears' ($null -ne $sheet)
    if (-not $sheet) { throw 'no wizard; cannot continue' }
    $sheetH = $sheet.Handle

    $page = Wait-ForPageControl -Sheet $sheetH -CtrlId $IDC_S1_COMBO
    Assert-That 'scene 1 is the first page shown' ($page -ne [IntPtr]::Zero)
    if ($page -eq [IntPtr]::Zero) { throw 'scene 1 never appeared' }
    Start-Sleep -Milliseconds 300
    Assert-Equal 'the keyboard starts on the language list' (Get-FocusedControlId $sheetH) $IDC_S1_COMBO

    # Larger than an 800x600 screen, to show more of a catalogue, yet within a
    # 1024x768 one above a 40 px taskbar. Measured at 100%: the installer is
    # per-monitor DPI aware, so its rectangle is real pixels at its own DPI.
    $frame = New-Object RECT
    [void][NativeUi]::GetWindowRect($sheetH, [ref]$frame)
    $scale = [NativeGeometry]::GetDpiForWindow($sheetH) / 96
    $width = [int](($frame.R - $frame.L) / $scale)
    $height = [int](($frame.B - $frame.T) / $scale)
    Assert-That 'the window is larger than an 800x600 screen' ($width -gt 800 -or $height -gt 600) `
        "${width}x$height at 100%"
    Assert-That 'and fits a 1024x768 screen above its taskbar' ($width -le 1024 -and $height -le 728) `
        "${width}x$height at 100%"

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

    # The way forward is painted light green so the eye finds it; nothing else is.
    $forward = Get-ControlFill -Window $sheetH -Control ([NativeUi]::GetDlgItem($sheetH, $ID_WIZNEXT))
    Assert-That 'the primary button is light green' (Test-IsGreen $forward) "painted $forward"
    $cancel = Get-ControlFill -Window $sheetH -Control ([NativeUi]::GetDlgItem($sheetH, 2))
    Assert-That 'Cancel keeps the theme''s look' (-not (Test-IsGreen $cancel)) "painted $cancel"

    # Choose the language under test, then continue.
    [void][NativeUi]::SendMessageW($combo, 0x014E, [IntPtr]$Language, [IntPtr]::Zero)
    Press-WizardButton -Sheet $sheetH -Button $PSBTN_NEXT

    Write-Host "`n-- scene 2: message of the day --"
    $page = Wait-ForPageControl -Sheet $sheetH -CtrlId $IDC_S2_MOTD
    Assert-That 'scene 2 follows scene 1' ($page -ne [IntPtr]::Zero)
    if ($page -eq [IntPtr]::Zero) { throw 'scene 2 never appeared' }
    Start-Sleep -Milliseconds 300
    Assert-Equal 'the keyboard starts on the secret field' (Get-FocusedControlId $sheetH) $IDC_S2_SECRET

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

    Write-Host "`n-- scene 3: game folder --"
    $page = Wait-ForPageControl -Sheet $sheetH -CtrlId $IDC_S3_PATH
    Assert-That 'scene 3 follows scene 2' ($page -ne [IntPtr]::Zero)
    if ($page -eq [IntPtr]::Zero) { throw 'scene 3 never appeared' }

    Assert-Equal 'scene 3 title' (Get-CtrlText $page $IDC_SCENE_TITLE) $S['IDS_S3_TITLE']
    Assert-Equal 'folder label' (Get-CtrlText $page $IDC_S3_PATH_LABEL) $S['IDS_S3_PATH']
    Assert-Equal 'game label' (Get-CtrlText $page $IDC_S3_GAME_LABEL) $S['IDS_S3_GAME']
    Assert-Equal 'browse button' (Get-CtrlText $page $IDC_S3_BROWSE) $S['IDS_BTN_BROWSE']
    Assert-Equal 'Back button is localized' `
    (Get-WizardButtonText -Sheet $sheetH -ButtonId $ID_WIZBACK) $S['IDS_BTN_BACK']
    Assert-That 'Back becomes available on scene 3 (DESIGN.md)' `
    (Test-WizardButtonEnabled -Sheet $sheetH -ButtonId $ID_WIZBACK)

    # The two routes are in-page buttons, and the sheet's Next is NOT the way
    # forward here: the routes are alternatives, so neither is the primary.
    Assert-Equal 'the Download/Update button is labelled' `
    (Get-CtrlText $page $IDC_S3_DOWNLOAD) $S['IDS_S3_DOWNLOAD']
    Assert-Equal 'the Configure/Install button is labelled' `
    (Get-CtrlText $page $IDC_S3_CONFIGURE) $S['IDS_S3_CONFIGURE']
    Assert-Equal 'the Restore/Uninstall button is labelled' `
    (Get-CtrlText $page $IDC_S3_RESTORE) $S['IDS_S3_RESTORE']
    Assert-AnyShipped 'the routes are explained' (Get-CtrlText $page $IDC_S3_ROUTES) 'IDS_S3_ROUTES'

    # Download/Update is the main flow, so it is the one that looks it: the
    # default button, the largest, and alone on the right. The two secondary
    # routes sit together on the left. Measured, because "looks primary" is
    # the whole point of the arrangement.
    $rects = @{}
    foreach ($id in $IDC_S3_CONFIGURE, $IDC_S3_RESTORE, $IDC_S3_DOWNLOAD) {
        $r = New-Object RECT
        [void][NativeUi]::GetWindowRect([NativeUi]::GetDlgItem($page, $id), [ref]$r)
        $rects[$id] = $r
    }
    $main = $rects[$IDC_S3_DOWNLOAD]
    $top = $rects[$IDC_S3_CONFIGURE]
    $bottom = $rects[$IDC_S3_RESTORE]
    Assert-That 'the two secondary routes share one column' ($top.L -eq $bottom.L) `
        "left edges $($top.L) and $($bottom.L)"
    Assert-That 'Restore/Uninstall sits under Configure/Install' ($bottom.T -ge $top.B) `
        "Configure ends at $($top.B), Restore starts at $($bottom.T)"
    Assert-That 'Download/Update is the right-hand column' ($main.L -gt $top.R) `
        "stack ends at $($top.R), the main button starts at $($main.L)"
    Assert-That 'and it is wider than they are' `
    (($main.R - $main.L) -gt ($top.R - $top.L)) `
        "main $(($main.R - $main.L)) wide, stack $(($top.R - $top.L))"
    # As tall as the pair, so the three form one clean block rather than
    # three buttons that happen to be near each other.
    Assert-That 'and as tall as the pair it stands beside' `
    ([Math]::Abs($main.T - $top.T) -le 2 -and [Math]::Abs($main.B - $bottom.B) -le 2) `
        "main $($main.T)..$($main.B), stack $($top.T)..$($bottom.B)"

    # Nothing entered yet: placeholder, disabled dropdown, no way forward.
    $gameCombo = [NativeUi]::GetDlgItem($page, $IDC_S3_GAME)
    $routeDown = [NativeUi]::GetDlgItem($page, $IDC_S3_DOWNLOAD)
    $routeConf = [NativeUi]::GetDlgItem($page, $IDC_S3_CONFIGURE)
    $routeRest = [NativeUi]::GetDlgItem($page, $IDC_S3_RESTORE)
    Assert-Equal 'dropdown starts on the detecting placeholder' `
    (Get-CtrlText $page $IDC_S3_GAME) $S['IDS_S3_DETECTING']
    Assert-That 'dropdown is disabled while nothing is detected' `
    (-not [NativeUi]::IsWindowEnabled($gameCombo))
    Assert-That "no route is open until a game is detected" `
    (-not [NativeUi]::IsWindowEnabled($routeDown) -and -not [NativeUi]::IsWindowEnabled($routeConf) -and
        -not [NativeUi]::IsWindowEnabled($routeRest))
    Assert-That 'the sheet Next is not a way out of scene 3' `
    (-not (Test-WizardButtonEnabled -Sheet $sheetH -ButtonId $ID_WIZNEXT))

    # A real folder that is not a game must not unlock anything.
    Set-FolderAndSettle -Page $page -Path $plainFolder
    Assert-Equal 'an ordinary folder leaves the placeholder' `
    (Get-CtrlText $page $IDC_S3_GAME) $S['IDS_S3_DETECTING']
    Assert-That "an ordinary folder leaves every route blocked" `
    (-not [NativeUi]::IsWindowEnabled($routeDown) -and -not [NativeUi]::IsWindowEnabled($routeConf) -and
        -not [NativeUi]::IsWindowEnabled($routeRest))

    # One marker: that game, shown but not selectable, and both routes open.
    Set-FolderAndSettle -Page $page -Path $rbrFolder
    Assert-Equal 'a single game is named in the dropdown' (Get-CtrlText $page $IDC_S3_GAME) $RBR_TITLE
    Assert-That 'dropdown stays disabled when there is nothing to choose' `
    (-not [NativeUi]::IsWindowEnabled($gameCombo))
    Assert-That "every route unlocks once a game is detected" `
    ([NativeUi]::IsWindowEnabled($routeDown) -and [NativeUi]::IsWindowEnabled($routeConf) -and
        [NativeUi]::IsWindowEnabled($routeRest))

    # The main route is this scene's way forward, so it is the green one; the
    # sheet's Next, disabled here, stays the theme's grey.
    $main = Get-ControlFill -Window $sheetH -Control $routeDown
    Assert-That 'Download/Update is light green' (Test-IsGreen $main) "painted $main"
    $side = Get-ControlFill -Window $sheetH -Control $routeConf
    Assert-That 'Configure/Install keeps the theme''s look' (-not (Test-IsGreen $side)) "painted $side"
    $idle = Get-ControlFill -Window $sheetH -Control ([NativeUi]::GetDlgItem($sheetH, $ID_WIZNEXT))
    Assert-That 'a disabled Next is not painted as the way forward' (-not (Test-IsGreen $idle)) "painted $idle"

    # Two markers: the dropdown becomes a real choice listing only the matches.
    Set-FolderAndSettle -Page $page -Path $twoGames
    $items = [int][NativeUi]::SendMessageW($gameCombo, 0x0146, [IntPtr]::Zero, [IntPtr]::Zero)
    Assert-That 'both detected games are offered' ($items -eq 2) "dropdown holds $items"
    Assert-That 'dropdown is selectable when the folder holds several games' `
    ([NativeUi]::IsWindowEnabled($gameCombo))
    Assert-Equal 'the first match is preselected' (Get-CtrlText $page $IDC_S3_GAME) $RBR_TITLE
    Assert-That "the routes stay available" `
    ([NativeUi]::IsWindowEnabled($routeDown) -and [NativeUi]::IsWindowEnabled($routeConf) -and
        [NativeUi]::IsWindowEnabled($routeRest))

    # Going back to nothing detected must re-lock the scene.
    Set-FolderAndSettle -Page $page -Path $plainFolder
    Assert-Equal 'clearing the detection restores the placeholder' `
    (Get-CtrlText $page $IDC_S3_GAME) $S['IDS_S3_DETECTING']
    Assert-That "and blocks every route again" `
    (-not [NativeUi]::IsWindowEnabled($routeDown) -and -not [NativeUi]::IsWindowEnabled($routeConf) -and
        -not [NativeUi]::IsWindowEnabled($routeRest))

    # A deny on writing for this account is what a standard user meets under
    # Program Files, made here without needing administrator rights (P11).
    Write-Host "`n-- a folder this account cannot write --"
    $locked = New-GameFolder -Markers @($RBR_MARKER)
    $deny = [System.Security.AccessControl.FileSystemAccessRule]::new(
        [System.Security.Principal.WindowsIdentity]::GetCurrent().Name, 'Write',
        'ContainerInherit, ObjectInherit', 'None', 'Deny')
    $acl = Get-Acl $locked; $acl.AddAccessRule($deny); Set-Acl -Path $locked -AclObject $acl
    try {
        Set-FolderAndSettle -Page $page -Path $locked
        Invoke-Button -Dialog $page -CtrlId $IDC_S3_DOWNLOAD -Post
        $modal = Wait-ForDialog -OwnerPid $proc.Id -NotHandle "$sheetH" -TimeoutMs 4000
        Assert-That 'a route into it stops with a message' ($null -ne $modal)
        if ($modal) {
            $said = (Get-ControlInventory -Dialog $modal.Handle |
                Where-Object { $_.Class -eq 'Static' -and $_.Text } | ForEach-Object { $_.Text }) -join ' '
            # Elevated already, there is nothing to offer, and the other text says so.
            $elevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).
            IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
            $id = if ($elevated) { 'IDS_S3_NOT_WRITABLE_ADMIN' } else { 'IDS_S3_NOT_WRITABLE' }
            $expected = Expand-ResourceArg -Template $S[$id] -Value $locked
            Assert-Equal "it says why, naming the folder ($id)" ($said -replace '\s+', ' ') ($expected -replace '\s+', ' ')
            if ($elevated) { Dismiss-Modal -Modal $modal.Handle } else { Decline-Modal -Modal $modal.Handle }
        }
        $still = Wait-ForPageControl -Sheet $sheetH -CtrlId $IDC_S3_PATH -TimeoutMs 3000
        Assert-That 'declining leaves the user on scene 3' ($still -ne [IntPtr]::Zero)
        Assert-That 'and the folder exactly as it was' (-not (Test-Path (Join-Path $locked '.pacenotes')))
    } finally {
        $acl = Get-Acl $locked; [void]$acl.RemoveAccessRule($deny); Set-Acl -Path $locked -AclObject $acl
        Remove-Item -Recurse -Force $locked -ErrorAction SilentlyContinue
    }

    Write-Host "`n-- back navigation --"
    Press-WizardButton -Sheet $sheetH -Button $PSBTN_BACK
    $back = Wait-ForPageControl -Sheet $sheetH -CtrlId $IDC_S2_MOTD -TimeoutMs 3000
    Assert-That 'Back returns to scene 2' ($back -ne [IntPtr]::Zero)
    Assert-Equal 'scene 2 primary button is restored' `
    (Get-WizardButtonText -Sheet $sheetH -ButtonId $ID_WIZNEXT) $S['IDS_BTN_ACCEPT']

    # What the elevated instance is started with, launched the same way: it
    # must resume on scene 3 rather than ask everything again (P11).
    Write-Host "`n-- restarted as administrator --"
    # Started while this instance is still open, as the real restart is: it
    # must wait for the old one to go rather than report "already running".
    # Start-Process, because Start-Installer refuses while a copy runs.
    $tag = if ($Language -eq 0) { 'en' } else { 'pl' }
    $old = $proc
    $proc = Start-Process $launch.Path -PassThru -ArgumentList @(
        '--server', 'http://127.0.0.1:1', '--restarted', '--language', $tag,
        '--folder', $rbrFolder, '--secret', 'pcn_0123456789abcdefghijklmnopqrstuvwxyzABCDEFG')
    Start-Sleep -Milliseconds 1500
    $old.Kill(); $old.WaitForExit(5000) | Out-Null
    $sheetR = Wait-ForDialog -OwnerPid $proc.Id
    Assert-That 'the restarted wizard appears' ($null -ne $sheetR)
    if ($sheetR) {
        $pageR = Wait-ForPageControl -Sheet $sheetR.Handle -CtrlId $IDC_S3_PATH -TimeoutMs 5000
        Assert-That 'it opens on scene 3' ($pageR -ne [IntPtr]::Zero)
        if ($pageR -ne [IntPtr]::Zero) {
            Start-Sleep -Milliseconds 900
            Assert-Equal 'in the language it was given' (Get-CtrlText $pageR $IDC_SCENE_TITLE) $S['IDS_S3_TITLE']
            Assert-Equal 'with the folder filled in' (Get-CtrlText $pageR $IDC_S3_PATH) $rbrFolder
            Assert-Equal 'and its game detected' (Get-CtrlText $pageR $IDC_S3_GAME) $RBR_TITLE
            Assert-That 'so a route can be taken at once' `
            ([NativeUi]::IsWindowEnabled([NativeUi]::GetDlgItem($pageR, $IDC_S3_DOWNLOAD)))
            # Keystrokes, since only the dialog manager decides these (I9).
            $hR = $sheetR.Handle
            Assert-Equal 'the keyboard starts on the folder field' (Get-FocusedControlId $hR) $IDC_S3_PATH
            $order = @()
            foreach ($i in 1..4) { Send-Key -Window $hR -VirtualKey $VK_TAB; $order += Get-FocusedControlId $hR }
            $expected = @($IDC_S3_BROWSE, $IDC_S3_DOWNLOAD, $IDC_S3_CONFIGURE, $IDC_S3_RESTORE)
            Assert-That 'Tab reaches the main route before the other two' `
            (-not (Compare-Object $order $expected -SyncWindow 0)) "Tab went $($order -join ' -> ')"
            Move-Focus -Sheet $hR -Control ([NativeUi]::GetDlgItem($pageR, $IDC_S3_PATH))
            Send-Key -Window $hR -VirtualKey $VK_RETURN
            $page4 = Wait-ForPageControl -Sheet $hR -CtrlId $IDC_S4_TREE -TimeoutMs 4000
            Assert-That 'Enter in the folder field takes Download/Update' ($page4 -ne [IntPtr]::Zero)
        }
    }
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
