<#
.SYNOPSIS
    AUTOMATED TEST. Walks the wizard to scenes 4 and 5 against a REAL server
    holding a nested catalogue, and asserts the Download and Configuration
    scenes. Exits 0/1.

.DESCRIPTION
    Test-UiLayout.ps1 covers scenes 1-3 against a DEAD server — the offline
    path, where scene 4 has nothing to show. This is the other half: a live
    server with packages filed the way an operator really files them, so the
    tree has every shape that matters (a package directly under its category,
    and voices nested two levels deep).

    What it does NOT re-assert: the tree's structure, ordering, check
    propagation and per-category validation are covered exhaustively and
    cross-platform by the unit tests in src/tree/tests.rs. Reading a tree view's items
    from another process needs the item struct written into that process's
    address space, which is a lot of machinery to re-prove what is already
    proven. What this adds is that the model reached the control at all, and
    that the gate the user hits is wired to it.

    Windows-only; needs a desktop session and a cargo toolchain.

.PARAMETER Language
    0 = English (default), 1 = Polish.

.EXAMPLE
    pwsh -File Test-WizardDownload.ps1
#>
[CmdletBinding()]
param([ValidateSet(0, 1)][int]$Language = 0, [string]$ExePath)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'Win32Ui.psm1') -Force

$IDC_SCENE_TITLE = 299
$IDC_S1_COMBO = 302
$IDC_S2_MOTD = 311; $IDC_S2_STATUS = 314
$IDC_S3_PATH = 322
$IDC_S3_DOWNLOAD = 326; $IDC_S3_CONFIGURE = 327; $IDC_S3_RESTORE = 329
$IDC_RESTORE_POINTS = 372; $IDC_RESTORE_PREVIEW = 373
$IDC_S4_TREE = 331; $IDC_S4_STATUS = 332
$IDC_S6_PHASE = 351
$IDC_S5_LANG_LABEL = 341; $IDC_S5_LANG = 342
$IDC_S5_VOICE_LABEL = 343; $IDC_S5_VOICE = 344
$IDC_S5_STYLE_LABEL = 345; $IDC_S5_STYLE = 346
$IDC_S7_RESULT = 361; $IDC_S7_DETAIL = 362; $IDC_S7_LOG = 363
$PSBTN_NEXT = 1; $PSBTN_FINISH = 2
$TVM_GETCOUNT = 0x1105; $TVM_GETNEXTITEM = 0x110A; $TVGN_ROOT = 0; $TVGN_NEXT = 1; $EM_GETLINECOUNT = 0x00BA

$crateRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$repoRoot = Split-Path -Parent $crateRoot
$server = Join-Path $repoRoot 'target\debug\pacenotes-server.exe'
$S = Get-ResourceStrings -Language $(if ($Language -eq 0) { 'en' } else { 'pl' })

$script:startedAt = Get-Date
$script:failures = New-Object System.Collections.ArrayList
$script:checks = 0
function Assert-That {
    param([Parameter(Mandatory)][string]$Name, [Parameter(Mandatory)][bool]$Condition, [string]$Detail = '')
    $script:checks++
    if ($Condition) { Write-Host "  PASS  $Name" -ForegroundColor Green }
    else {
        Write-Host ("  FAIL  {0}{1}" -f $Name, $(if ($Detail) { " -- $Detail" } else { '' })) -ForegroundColor Red
        [void]$script:failures.Add($Name)
    }
}
function Assert-Equal {
    param([string]$Name, [AllowNull()][string]$Actual, [string]$Expected)
    Assert-That -Name $Name -Condition ($Actual -ceq $Expected) -Detail "expected [$Expected], got [$Actual]"
}
function Get-FreePort {
    $l = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $l.Start(); $p = $l.LocalEndpoint.Port; $l.Stop(); return $p
}

<#  The catalogue, filed the way the operator files it: the folders ARE the
    tree. Two shapes matter — a package sitting directly under its category,
    and co-driver voices nested language/gender deep. #>
$CATALOGUE = @(
    'style/Descriptive', 'style/Numeric',
    'language/english', 'language/polish',
    'pacenotes/RBR',
    'codriver/polish/male/Kamil', 'codriver/polish/female/Ewa',
    'codriver/english/male/John'
)
# 4 categories + codriver's 2 languages + 3 gender rows + 8 leaves.
$EXPECTED_ROWS = 17

Write-Host "=== Wizard Download scene (language index $Language) ===" -ForegroundColor Cyan

Write-Host 'building pacenotes-server...'
Push-Location $repoRoot
$savedRustflags = $env:RUSTFLAGS
$env:RUSTFLAGS = $null
try {
    cargo build -p pacenotes-server 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { Write-Error 'server build failed'; exit 1 }
} finally { $env:RUSTFLAGS = $savedRustflags; Pop-Location }

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("pacenotes-s4-" + [guid]::NewGuid())
$data = Join-Path $work 'data'
$proc = $null; $srvProc = $null
try {
    $stage = Join-Path $work 'stage\Plugins\Pacenote\config'
    [void](New-Item -ItemType Directory -Force -Path $stage)
    Set-Content -Path (Join-Path $stage 'x.ini') -Value 'overlay'
    foreach ($rel in $CATALOGUE) {
        $into = Join-Path $data ("rbr\packages\public\" + ((Split-Path $rel -Parent) -replace '/', '\'))
        [void](New-Item -ItemType Directory -Force -Path $into)
        Compress-Archive -Path (Join-Path $work 'stage\Plugins') `
            -DestinationPath (Join-Path $into ((Split-Path $rel -Leaf) + '.zip')) -Force
    }
    # A game without a schema publishes nothing, and this catalogue uses exactly RBR's.
    Copy-Item -LiteralPath (Join-Path $repoRoot 'schemas\rbr.toml') `
        -Destination (Join-Path $data 'rbr\schema.toml')
    # Written on Linux or in any editor set to LF, a message of the day ends its lines in LF only.
    [void](New-Item -ItemType Directory -Force -Path (Join-Path $data 'motd'))
    $MOTD_LINES = @('Rally season opens on Friday.', 'New voices this week:', 'Kamil and Ewa.')
    [System.IO.File]::WriteAllText((Join-Path $data 'motd\en.txt'), ($MOTD_LINES -join "`n") + "`n")
    & $server manifest gen --data $data 2>&1 | Out-Null

    $port = Get-FreePort
    $srvProc = Start-Process $server -PassThru -WindowStyle Hidden -ArgumentList @(
        'serve', '--listen', "127.0.0.1:$port", '--data', $data, '--db-url', "file:$(Join-Path $work 'auth.db')")

    # A folder that really is an RBR install, so scene 3 lets us through.
    $gameFolder = Join-Path $work 'game'
    [void](New-Item -ItemType Directory -Force -Path $gameFolder)
    [void](New-Item -ItemType File -Path (Join-Path $gameFolder 'RichardBurnsRally_SSE.exe'))

    # No --cache, since a run that overrode the path would not test where downloads land.
    $cacheDir = Join-Path $gameFolder '.pacenotes\cache'
    $launch = Start-Installer -ExePath $ExePath -ExtraArgs @(
        '--server', "http://127.0.0.1:$port")
    $proc = $launch.Process

    $sheet = Wait-ForDialog -OwnerPid $proc.Id
    Assert-That 'wizard appears' ($null -ne $sheet)
    if (-not $sheet) { throw 'no wizard' }
    $h = $sheet.Handle

    $page = Wait-ForPageControl -Sheet $h -CtrlId $IDC_S1_COMBO
    $combo = [NativeUi]::GetDlgItem($page, $IDC_S1_COMBO)
    [void][NativeUi]::SendMessageW($combo, 0x014E, [IntPtr]$Language, [IntPtr]::Zero)
    Press-WizardButton -Sheet $h -Button $PSBTN_NEXT

    $page = Wait-ForPageControl -Sheet $h -CtrlId $IDC_S2_MOTD
    $deadline = (Get-Date).AddSeconds(15)
    do {
        Start-Sleep -Milliseconds 300
        $status = Get-CtrlText $page $IDC_S2_STATUS
    } while ($status -cne $S['IDS_S2_ONLINE'] -and (Get-Date) -lt $deadline)
    Assert-Equal 'server is seen as online' $status $S['IDS_S2_ONLINE']
    # Polish falls back to the English file, so both languages see it.
    $deadline = (Get-Date).AddSeconds(10)
    while (-not (Get-CtrlText $page $IDC_S2_MOTD) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 200 }
    $motd = [NativeUi]::GetDlgItem($page, $IDC_S2_MOTD)
    $motdLines = [int][NativeUi]::SendMessageW($motd, $EM_GETLINECOUNT, [IntPtr]::Zero, [IntPtr]::Zero)
    Assert-That 'the message of the day keeps the line breaks of its file' `
    ($motdLines -eq $MOTD_LINES.Count) "the pane shows $motdLines line(s), the file has $($MOTD_LINES.Count)"
    Assert-That 'and every line of it' ((Get-CtrlText $page $IDC_S2_MOTD) -ceq ($MOTD_LINES -join "`r`n")) `
        "the pane says [$(Get-CtrlText $page $IDC_S2_MOTD)]"
    Press-WizardButton -Sheet $h -Button $PSBTN_NEXT

    $page = Wait-ForPageControl -Sheet $h -CtrlId $IDC_S3_PATH
    Set-CtrlText -Dialog $page -CtrlId $IDC_S3_PATH -Value $gameFolder
    Invoke-EditChanged -Dialog $page -CtrlId $IDC_S3_PATH
    Start-Sleep -Milliseconds 900
    # The Download/Update route, which is one of scene 3's two buttons — not
    # the sheet's Next, which that scene leaves disabled.
    Invoke-Button -Dialog $page -CtrlId $IDC_S3_DOWNLOAD

    Write-Host "`n-- scene 4: download --"
    $page = Wait-ForPageControl -Sheet $h -CtrlId $IDC_S4_TREE
    Assert-That 'scene 4 follows scene 3' ($page -ne [IntPtr]::Zero)
    if ($page -eq [IntPtr]::Zero) { throw 'scene 4 never appeared' }

    Assert-Equal 'scene 4 title' (Get-CtrlText $page $IDC_SCENE_TITLE) $S['IDS_S4_TITLE']
    Assert-Equal 'primary button reads Download' `
    (Get-WizardButtonText -Sheet $h -ButtonId 0x3024) $S['IDS_BTN_DOWNLOAD']

    # The catalogue arrives from the worker thread.
    $tv = [NativeUi]::GetDlgItem($page, $IDC_S4_TREE)
    $deadline = (Get-Date).AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 300
        $rows = [int][NativeUi]::SendMessageW($tv, $TVM_GETCOUNT, [IntPtr]::Zero, [IntPtr]::Zero)
    } while ($rows -eq 0 -and (Get-Date) -lt $deadline)

    Assert-Equal 'status says the tree is ready' (Get-CtrlText $page $IDC_S4_STATUS) $S['IDS_S4_READY']
    # Every grouping row the operator's folders imply, plus every package.
    Assert-That 'the whole catalogue reached the tree control' ($rows -eq $EXPECTED_ROWS) `
        "control holds $rows rows, expected $EXPECTED_ROWS"

    # REGRESSION (2026-11-01): Back, Configure/Install, Back, Download/Update,
    # on a folder with nothing downloaded yet. The Configure route replaced
    # the model behind scene 4's control, so the next visit let Download
    # through with nothing ticked into an empty scene 5, and the one after
    # locked the page. Twice, as reported. Configure must refuse here, with
    # nothing on this computer to choose from; the gate and the ticks that
    # follow then prove scene 4's model is the catalogue again.
    $notReady = $S['IDS_S5_NOT_READY'].Substring(0, $S['IDS_S5_NOT_READY'].IndexOf('%s')).Trim()
    foreach ($round in 1, 2) {
        Write-Host "`n-- Back, Configure/Install, Back, Download/Update ($round) --"
        Press-WizardButton -Sheet $h -Button $PSBTN_BACK
        $p3 = Wait-ForPageControl -Sheet $h -CtrlId $IDC_S3_PATH
        Invoke-Button -Dialog $p3 -CtrlId $IDC_S3_CONFIGURE -Post
        $refusal = Wait-ForDialog -OwnerPid $proc.Id -NotHandle "$h" -TimeoutMs 4000
        Assert-That "Configure/Install with nothing downloaded is refused ($round)" ($null -ne $refusal)
        if ($refusal) {
            $said = (Get-ControlInventory -Dialog $refusal.Handle |
                Where-Object { $_.Class -eq 'Static' } | ForEach-Object { $_.Text }) -join ' '
            Assert-That "the refusal says there is nothing here to choose from ($round)" `
            ($said.Contains($notReady)) "warning said [$said]"
            foreach ($caption in 'IDS_S5_LANGUAGE', 'IDS_S5_VOICE', 'IDS_S5_STYLE') {
                $named = $S[$caption].TrimEnd(':')
                Assert-That "and names '$named' ($round)" ($said.Contains($named)) "warning said [$said]"
            }
            Dismiss-Modal -Modal $refusal.Handle
        }
        $p3 = Wait-ForPageControl -Sheet $h -CtrlId $IDC_S3_PATH -TimeoutMs 3000
        Assert-That "the wizard stays on scene 3 ($round)" ($p3 -ne [IntPtr]::Zero)
        if ($p3 -eq [IntPtr]::Zero) { throw 'Configure/Install left scene 3 with nothing downloaded' }

        Invoke-Button -Dialog $p3 -CtrlId $IDC_S3_DOWNLOAD
        $page = Wait-ForPageControl -Sheet $h -CtrlId $IDC_S4_TREE
        $deadline = (Get-Date).AddSeconds(20)
        do {
            Start-Sleep -Milliseconds 300
            $status = Get-CtrlText $page $IDC_S4_STATUS
        } while ($status -cne $S['IDS_S4_READY'] -and (Get-Date) -lt $deadline)
        $rows = Get-TreeRowCount -Dialog $page -CtrlId $IDC_S4_TREE
        Assert-That "scene 4 shows the whole catalogue again ($round)" ($rows -eq $EXPECTED_ROWS) `
            "control holds $rows rows, expected $EXPECTED_ROWS"
    }

    # Only the choice categories schemas/rbr.toml marks select = "one" are demanded.
    Write-Host "`n-- download gate --"
    Press-WizardButton -Sheet $h -Button $PSBTN_NEXT
    $modal = Wait-ForDialog -OwnerPid $proc.Id -NotHandle "$h" -TimeoutMs 4000
    Assert-That 'Download with nothing ticked raises a warning' ($null -ne $modal)
    if ($modal) {
        # The warning must NAME the categories, or the user cannot act on it.
        $said = (Get-ControlInventory -Dialog $modal.Handle |
            Where-Object { $_.Class -eq 'Static' } | ForEach-Object { $_.Text }) -join ' '
        foreach ($category in 'language', 'codriver', 'style') {
            Assert-That "warning names the missing category '$category'" `
            ($said -clike "*$category*") "warning said [$said]"
        }
        Assert-That "warning leaves the optional 'pacenotes' category out" `
        (-not ($said -clike '*pacenotes*')) "warning said [$said]"
        Dismiss-Modal -Modal $modal.Handle
    }
    $still = Wait-ForPageControl -Sheet $h -CtrlId $IDC_S4_TREE -TimeoutMs 3000
    Assert-That 'and the wizard stays on scene 4' ($still -ne [IntPtr]::Zero)

    # Ticking every category is what the gate asked for.
    $ticked = Invoke-TreeCheckRoots -Dialog $page -CtrlId $IDC_S4_TREE
    Assert-That 'every category can be ticked' ($ticked -eq 4) "ticked $ticked roots, expected 4"
    Press-WizardButton -Sheet $h -Button $PSBTN_NEXT

    # Scene 4b runs itself, and catching it on screen would race, so what we wait for is scene 5.
    Write-Host "`n-- scene 5: configuration --"
    $page5 = Wait-ForPageControl -Sheet $h -CtrlId $IDC_S5_LANG -TimeoutMs 40000
    Assert-That 'a tick in every category gets past the gate' ($page5 -ne [IntPtr]::Zero)
    if ($page5 -eq [IntPtr]::Zero) { throw 'scene 5 never appeared' }

    # THE ordering the two screens exist for: by the time the questions are
    # asked the payloads are downloaded, and the game folder has not been
    # touched. A ledger here would mean the install ran first.
    # Counted from the cache INDEX, not the payload files: the cache is
    # sha-keyed, and every zip in this fixture is built from the same staged
    # folder, so all four packages legitimately share one payload.
    $indexPath = Join-Path $cacheDir 'cache.json'
    $indexed = if (Test-Path $indexPath) {
        @((Get-Content $indexPath -Raw | ConvertFrom-Json).packages.PSObject.Properties.Name)
    } else { @() }
    Assert-That 'the payloads are downloaded before the questions are asked' `
    ($indexed.Count -eq $CATALOGUE.Count) "cache index holds $($indexed.Count) packages"
    Assert-That 'nothing is installed before the questions are answered' `
    (-not (Test-Path (Join-Path $gameFolder '.pacenotes\installed.json')))

    Assert-Equal 'scene 5 title' (Get-CtrlText $page5 $IDC_SCENE_TITLE) $S['IDS_S5_TITLE']
    Assert-Equal 'primary button reads Install' `
    (Get-WizardButtonText -Sheet $h -ButtonId 0x3024) $S['IDS_BTN_INSTALL']
    Assert-Equal 'language list caption' (Get-CtrlText $page5 $IDC_S5_LANG_LABEL) $S['IDS_S5_LANGUAGE']
    Assert-Equal 'voice list caption' (Get-CtrlText $page5 $IDC_S5_VOICE_LABEL) $S['IDS_S5_VOICE']
    Assert-Equal 'style list caption' (Get-CtrlText $page5 $IDC_S5_STYLE_LABEL) $S['IDS_S5_STYLE']

    # Expected counts come from $CATALOGUE, so the fixture cannot go stale against them.
    $perCategory = @{}
    foreach ($rel in $CATALOGUE) {
        $category = ($rel -split '/')[0]
        $perCategory[$category] = 1 + $(if ($perCategory.ContainsKey($category)) { $perCategory[$category] } else { 0 })
    }
    foreach ($pair in @(
            @{ Name = 'language'; Id = $IDC_S5_LANG },
            @{ Name = 'codriver'; Id = $IDC_S5_VOICE },
            @{ Name = 'style'; Id = $IDC_S5_STYLE })) {
        $count = Get-ListCount -Dialog $page5 -CtrlId $pair.Id
        Assert-That "the $($pair.Name) list offers every ticked package" `
        ($count -eq $perCategory[$pair.Name]) `
            "list holds $count, catalogue has $($perCategory[$pair.Name])"
        # With several options the installer must not choose for the user.
        Assert-That "nothing is preselected in the $($pair.Name) list" `
        ((Get-ListSelection -Dialog $page5 -CtrlId $pair.Id) -eq -1)
    }

    Write-Host "`n-- install gate --"
    Press-WizardButton -Sheet $h -Button $PSBTN_NEXT
    $modal5 = Wait-ForDialog -OwnerPid $proc.Id -NotHandle "$h" -TimeoutMs 4000
    Assert-That 'Install with nothing chosen raises a warning' ($null -ne $modal5)
    if ($modal5) {
        $said = (Get-ControlInventory -Dialog $modal5.Handle |
            Where-Object { $_.Class -eq 'Static' } | ForEach-Object { $_.Text }) -join ' '
        foreach ($caption in 'IDS_S5_LANGUAGE', 'IDS_S5_VOICE', 'IDS_S5_STYLE') {
            # The modal names each list by its label without the colon.
            $named = $S[$caption].TrimEnd(':')
            Assert-That "warning names '$named'" ($said.Contains($named)) "warning said [$said]"
        }
        Dismiss-Modal -Modal $modal5.Handle
    }

    # Answering all three satisfies the gate.
    foreach ($pair in @(
            @{ Name = 'language'; Id = $IDC_S5_LANG },
            @{ Name = 'codriver'; Id = $IDC_S5_VOICE },
            @{ Name = 'style'; Id = $IDC_S5_STYLE })) {
        $selected = Select-ListItem -Dialog $page5 -CtrlId $pair.Id -Index 1
        Assert-That "a $($pair.Name) can be chosen" ($selected -eq 1) "selection came back $selected"
    }
    Press-WizardButton -Sheet $h -Button $PSBTN_NEXT

    Write-Host "`n-- scenes 6 and 7: installation and summary --"
    # Scene 7 appearing proves Install was accepted, which needs every pick in the model.
    $page7 = Wait-ForPageControl -Sheet $h -CtrlId $IDC_S7_RESULT -TimeoutMs 40000
    Assert-That 'Install with all three chosen runs and reaches the summary' `
    ($page7 -ne [IntPtr]::Zero)
    if ($page7 -eq [IntPtr]::Zero) { throw 'scene 7 never appeared' }

    Assert-Equal 'scene 7 title' (Get-CtrlText $page7 $IDC_SCENE_TITLE) $S['IDS_S7_TITLE']
    Assert-Equal 'primary button reads Finish' `
    (Get-WizardButtonText -Sheet $h -ButtonId 0x3024) $S['IDS_BTN_FINISH']
    Assert-Equal 'the result line reports success' `
    (Get-CtrlText $page7 $IDC_S7_RESULT) $S['IDS_S7_OK']

    $detail = Get-CtrlText $page7 $IDC_S7_DETAIL
    Assert-That 'the summary names the game folder' ($detail.Contains($gameFolder)) `
        "summary said [$detail]"
    foreach ($caption in 'IDS_S5_LANGUAGE', 'IDS_S5_VOICE', 'IDS_S5_STYLE') {
        Assert-That "the summary reports the $($S[$caption].TrimEnd(':')) pick" `
        ($detail.Contains($S[$caption])) "summary said [$detail]"
    }
    # Every package was ticked and the cache started empty.
    Assert-That 'the summary counts what was downloaded' `
    ($detail.Contains("$($CATALOGUE.Count) (")) "summary said [$detail]"

    # DESIGN.md collapses the log on success; hidden, it still holds the record.
    Assert-That 'the log pane is collapsed on success' `
    (-not (Test-CtrlVisible -Dialog $page7 -CtrlId $IDC_S7_LOG))
    $logText = Get-CtrlText $page7 $IDC_S7_LOG
    Assert-That 'the log recorded the download phase' `
    ($logText.Contains('downloading rbr/public/')) "log said [$logText]"
    Assert-That 'the log recorded the install phase' `
    ($logText.Contains('installed rbr/public/')) "log said [$logText]"
    Assert-That 'the log names the restore point' `
    ($logText.Contains('restore point')) "log said [$logText]"

    # The files are in the game folder, and the ledger records them.
    Write-Host "`n-- the game folder --"
    $ledgerPath = Join-Path $gameFolder '.pacenotes\installed.json'
    Assert-That 'the ledger was written' (Test-Path $ledgerPath)
    $ledger = Get-Content $ledgerPath -Raw | ConvertFrom-Json
    $recorded = @($ledger.packages.PSObject.Properties.Name)
    Assert-That 'every configured package is in the ledger' ($recorded.Count -eq 4) `
        "ledger holds $($recorded.Count): $($recorded -join ', ')"
    Assert-That 'the overlay really landed in the game folder' `
    (Test-Path (Join-Path $gameFolder 'Plugins\Pacenote\config\x.ini'))
    Assert-That 'a restore point was taken' `
    (@(Get-ChildItem (Join-Path $gameFolder '.pacenotes\restore') -ErrorAction SilentlyContinue).Count -ge 1)

    # The per-game step: RBR writes the plugin's settings file.
    $settings = Join-Path $gameFolder 'Plugins\Pacenote\PaceNote.ini'
    Assert-That 'the RBR configuration step wrote the settings file' (Test-Path $settings)
    Assert-That 'and set the language the user picked' `
    ((Get-Content $settings -Raw).Contains('language=polish')) `
        "settings said [$(Get-Content $settings -Raw)]"

    $payloads = @(Get-ChildItem -Path $cacheDir -Filter '*.zip' -ErrorAction SilentlyContinue)
    Assert-That 'the verified payloads are in the cache' ($payloads.Count -ge 1) `
        "cache holds $($payloads.Count) payloads"
    Assert-That 'the cache index was written' (Test-Path (Join-Path $cacheDir 'cache.json'))

    # NOTHING outside the game folder. The whole point of putting the cache
    # there: deleting the game takes everything this installer wrote with it.
    Assert-That 'the downloads went into the game folder, not the profile' `
    (Test-Path (Join-Path $gameFolder '.pacenotes\cache\cache.json'))
    # Not "does it exist" — a developer's machine has one left over from
    # the builds that wrote there. "Did THIS run touch it" is the question.
    $profileCache = Join-Path $env:LOCALAPPDATA 'PacenotesInstaller'
    $touched = (Test-Path $profileCache) -and
        ((Get-Item $profileCache).LastWriteTime -gt $script:startedAt)
    Assert-That 'and nothing was written to the per-user profile' (-not $touched) `
        "$profileCache was modified during the run"

    # The log lives WITH the game, not in the user's profile.
    $runLogs = @(Get-ChildItem -Path (Join-Path $gameFolder '.pacenotes\logs') `
            -Filter 'install-*.log' -ErrorAction SilentlyContinue)
    Assert-That 'the run log was written into the game folder' ($runLogs.Count -eq 1) `
        "found $($runLogs.Count) logs"
    if ($runLogs.Count -eq 1) {
        $onDisk = Get-Content $runLogs[0].FullName -Raw
        # It is the WHOLE run, including the lines from before the folder was
        # known — the log attaches mid-run and replays what it has.
        Assert-That 'the file holds the whole run, from before the folder was known' `
        ($onDisk.Contains('game: ') -and $onDisk.Contains('installed rbr/public/')) `
            "log file said [$onDisk]"
        Assert-That 'the summary points at that file' `
        ($detail.Contains($runLogs[0].FullName)) "summary said [$detail]"

        # What support asks for first: which build, when, which server, which Windows.
        $logLines = @(Get-Content $runLogs[0].FullName)
        $build = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($launch.Path).ProductVersion
        Assert-That 'the log opens by naming this build' `
        ($logLines[0] -eq "Pacenotes Installer $build") "first line [$($logLines[0])]"
        Assert-That 'and says when the run started, in UTC' `
        ($onDisk -match '(?m)^started  \d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$')
        Assert-That 'and names the server the run used' `
        ($onDisk -match "(?m)^server   http://127\.0\.0\.1:$port$")
        $os = [Environment]::OSVersion.Version
        Assert-That 'and the real Windows build, not the compatibility one' `
        ($onDisk -match "(?m)^windows  $($os.Major)\.$($os.Minor)\.$($os.Build)$") `
            "expected windows $($os.Major).$($os.Minor).$($os.Build)"
        $blank = [array]::IndexOf($logLines, '')
        $body = @($logLines | Select-Object -Skip ($blank + 1))
        $untimed = @($body | Where-Object { $_ -notmatch '^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ  ' })
        Assert-That 'every line after the header says when it was written' `
        (($blank -gt 0) -and $body.Count -gt 0 -and $untimed.Count -eq 0) "untimed: [$($untimed -join ' | ')]"
        Assert-That 'while the pane shows the same lines without header or times' `
        ($logText.Contains('installed rbr/public/') -and -not $logText.Contains('started  ') -and
            -not ($logText -match '\d\d:\d\d:\d\dZ  ')) "pane said [$logText]"
    }

    # ---------------------------------------------------------------
    # Second pass — the Configure/Install route
    # ---------------------------------------------------------------
    # The same folder and the same cache, so there is nothing to download:
    # this is the route for "change what is installed", and it must skip
    # both download screens and still reach a working install.
    Write-Host "`n-- second pass: Configure/Install --"
    # Finish, not Next: scene 7 shows the Finish button, and PSM_PRESSBUTTON
    # addresses whichever of the two is live by its own PSBTN_* code.
    Press-WizardButton -Sheet $h -Button $PSBTN_FINISH
    Assert-That 'Finish closes the first run' ($proc.WaitForExit(10000))
    # What a download killed mid-way leaves behind; the next run's sweep removes it (I9).
    $stray = Join-Path $cacheDir 'abandoned.part'
    Set-Content -Path $stray -Value 'half a package'

    $launch2 = Start-Installer -ExePath $ExePath -ExtraArgs @(
        '--server', "http://127.0.0.1:$port")
    $proc = $launch2.Process
    $sheet2 = Wait-ForDialog -OwnerPid $proc.Id
    Assert-That 'the second run opens' ($null -ne $sheet2)
    if (-not $sheet2) { throw 'no wizard on the second pass' }
    $h2 = $sheet2.Handle

    $p = Wait-ForPageControl -Sheet $h2 -CtrlId $IDC_S1_COMBO
    $combo2 = [NativeUi]::GetDlgItem($p, $IDC_S1_COMBO)
    [void][NativeUi]::SendMessageW($combo2, 0x014E, [IntPtr]$Language, [IntPtr]::Zero)
    Press-WizardButton -Sheet $h2 -Button $PSBTN_NEXT
    $p = Wait-ForPageControl -Sheet $h2 -CtrlId $IDC_S2_MOTD
    Press-WizardButton -Sheet $h2 -Button $PSBTN_NEXT
    $p = Wait-ForPageControl -Sheet $h2 -CtrlId $IDC_S3_PATH
    Set-CtrlText -Dialog $p -CtrlId $IDC_S3_PATH -Value $gameFolder
    Invoke-EditChanged -Dialog $p -CtrlId $IDC_S3_PATH
    Start-Sleep -Milliseconds 900
    Invoke-Button -Dialog $p -CtrlId $IDC_S3_CONFIGURE

    # Straight to the questions: no tree, no progress bar in between.
    $conf = Wait-ForPageControl -Sheet $h2 -CtrlId $IDC_S5_LANG -TimeoutMs 15000
    Assert-That 'Configure/Install goes straight to the questions' ($conf -ne [IntPtr]::Zero)
    if ($conf -ne [IntPtr]::Zero) {
        # Every cached package is on offer, because this route ticks what is
        # already here.
        $offered = Get-ListCount -Dialog $conf -CtrlId $IDC_S5_LANG
        Assert-That 'the lists are filled from the cache and the ledger' ($offered -ge 1) `
            "language list holds $offered"
        # What was installed last time is preselected: a run that omitted it
        # would UNINSTALL it, so "keep what you have" has to be the default.
        Assert-That 'last run''s pick is preselected' `
        ((Get-ListSelection -Dialog $conf -CtrlId $IDC_S5_LANG) -ge 0)

        Press-WizardButton -Sheet $h2 -Button $PSBTN_NEXT
        $done = Wait-ForPageControl -Sheet $h2 -CtrlId $IDC_S7_RESULT -TimeoutMs 40000
        Assert-That 'the Configure/Install route reaches the summary' ($done -ne [IntPtr]::Zero)
        if ($done -ne [IntPtr]::Zero) {
            Assert-Equal 'and reports success' (Get-CtrlText $done $IDC_S7_RESULT) $S['IDS_S7_OK']
            $detail2 = Get-CtrlText $done $IDC_S7_DETAIL
            Assert-That 'with nothing downloaded' `
            ($detail2.Contains($S['IDS_S7_NOTHING_TODO'])) "summary said [$detail2]"
        }
        # A second run, a second log file — the first one is still readable.
        $logs2 = @(Get-ChildItem -Path (Join-Path $gameFolder '.pacenotes\logs') `
                -Filter 'install-*.log' -ErrorAction SilentlyContinue)
        Assert-That 'the second run kept the first run''s log' ($logs2.Count -eq 2) `
            "found $($logs2.Count) logs"
        Assert-That 'the half-finished download was swept from the cache' (-not (Test-Path $stray))
        $newestLog = $logs2 | Sort-Object Name | Select-Object -Last 1
        Assert-That 'and the run log says so' `
        ((Get-Content $newestLog.FullName -Raw).Contains('cache: removed 1 file(s)'))
        Assert-That 'while every payload the folder uses stayed' `
        (@(Get-ChildItem -Path $cacheDir -Filter '*.zip').Count -gt 0)
    }

    # ---------------------------------------------------------------
    # Third pass — switching routes mid-run
    # ---------------------------------------------------------------
    # REGRESSION (2026-10-26): Configure/Install, change a pick, Back, then
    # Download/Update aborted the process. The Configure route loads a tree
    # model without a tree CONTROL to mirror it into; the control then sent
    # an item-changed notification carrying lParam 0 — indistinguishable
    # from "node 0" — and the posted refresh indexed a handle table that was
    # still empty. The crash was silent, because a GUI binary with
    # panic = "abort" has nowhere to print.
    Write-Host "`n-- third pass: Configure/Install -> Back -> Download/Update --"
    Press-WizardButton -Sheet $h2 -Button $PSBTN_FINISH
    Assert-That 'Finish closes the second run' ($proc.WaitForExit(10000))

    $launch3 = Start-Installer -ExePath $ExePath -ExtraArgs @(
        '--server', "http://127.0.0.1:$port")
    $proc = $launch3.Process
    $sheet3 = Wait-ForDialog -OwnerPid $proc.Id
    if (-not $sheet3) { throw 'no wizard on the third pass' }
    $h3 = $sheet3.Handle

    $p = Wait-ForPageControl -Sheet $h3 -CtrlId $IDC_S1_COMBO
    $combo3 = [NativeUi]::GetDlgItem($p, $IDC_S1_COMBO)
    [void][NativeUi]::SendMessageW($combo3, 0x014E, [IntPtr]$Language, [IntPtr]::Zero)
    Press-WizardButton -Sheet $h3 -Button $PSBTN_NEXT
    $p = Wait-ForPageControl -Sheet $h3 -CtrlId $IDC_S2_MOTD
    Press-WizardButton -Sheet $h3 -Button $PSBTN_NEXT
    $p = Wait-ForPageControl -Sheet $h3 -CtrlId $IDC_S3_PATH
    Set-CtrlText -Dialog $p -CtrlId $IDC_S3_PATH -Value $gameFolder
    Invoke-EditChanged -Dialog $p -CtrlId $IDC_S3_PATH
    Start-Sleep -Milliseconds 900

    Invoke-Button -Dialog $p -CtrlId $IDC_S3_CONFIGURE
    $conf3 = Wait-ForPageControl -Sheet $h3 -CtrlId $IDC_S5_LANG -TimeoutMs 15000
    Assert-That 'the questions open on the Configure route' ($conf3 -ne [IntPtr]::Zero)
    if ($conf3 -ne [IntPtr]::Zero) {
        # Change the answer: the reported sequence did, and a changed pick is
        # what the user would have lost if the model had been reset.
        $was = Get-ListSelection -Dialog $conf3 -CtrlId $IDC_S5_LANG
        $other = if ($was -eq 0) { 1 } else { 0 }
        [void](Select-ListItem -Dialog $conf3 -CtrlId $IDC_S5_LANG -Index $other)

        # Back goes to the game folder, not into a page this route skipped.
        Press-WizardButton -Sheet $h3 -Button $PSBTN_BACK
        $back3 = Wait-ForPageControl -Sheet $h3 -CtrlId $IDC_S3_PATH -TimeoutMs 8000
        Assert-That 'Back from the questions returns to the game folder' `
        ($back3 -ne [IntPtr]::Zero)

        # ...and the other route still works afterwards.
        Invoke-Button -Dialog $back3 -CtrlId $IDC_S3_DOWNLOAD
        $tree3 = Wait-ForPageControl -Sheet $h3 -CtrlId $IDC_S4_TREE -TimeoutMs 20000
        Assert-That 'Download/Update survives the route switch' ($tree3 -ne [IntPtr]::Zero) `
            'the installer died opening scene 4'
        Assert-That 'the process is still alive' (-not $proc.HasExited) `
            $(if ($proc.HasExited) { "exit 0x$($proc.ExitCode.ToString('X8'))" } else { '' })
        if ($tree3 -ne [IntPtr]::Zero) {
            $deadline = (Get-Date).AddSeconds(20)
            do {
                Start-Sleep -Milliseconds 300
                $rows3 = [int][NativeUi]::SendMessageW(
                    [NativeUi]::GetDlgItem($tree3, $IDC_S4_TREE), $TVM_GETCOUNT,
                    [IntPtr]::Zero, [IntPtr]::Zero)
            } while ($rows3 -eq 0 -and (Get-Date) -lt $deadline)
            Assert-That 'the catalogue is there' ($rows3 -eq $EXPECTED_ROWS) `
                "control holds $rows3 rows, expected $EXPECTED_ROWS"
            # Ticking is what posts the refresh that used to abort.
            $ticked3 = Invoke-TreeCheckRoots -Dialog $tree3 -CtrlId $IDC_S4_TREE
            Start-Sleep -Milliseconds 500
            Assert-That 'and ticking still works' `
            ($ticked3 -eq 4 -and -not $proc.HasExited) "ticked $ticked3 roots"
        }
        # No crash line anywhere: the hook writes one if a panic happened,
        # even when the window has already gone.
        $crashBeside = Join-Path (Split-Path $launch3.Path -Parent) `
            'pacenotes-installer-crash.log'
        Assert-That 'nothing was written to the crash log' (-not (Test-Path $crashBeside)) `
            $(if (Test-Path $crashBeside) { (Get-Content $crashBeside -Raw) } else { '' })
    }

    # ---------------------------------------------------------------
    # Fourth pass — Restore/Uninstall
    # ---------------------------------------------------------------
    # The folder now has real installs and the restore points the runs
    # above left behind. This is the route that undoes them, and the one
    # place the installer can REMOVE a user's files, so what it checks is
    # that the preview says what will happen and that what happens is
    # what it said.
    Write-Host "`n-- fourth pass: Restore/Uninstall --"
    $proc.Kill(); $proc.WaitForExit(5000) | Out-Null

    $launch4 = Start-Installer -ExePath $ExePath -ExtraArgs @(
        '--server', "http://127.0.0.1:$port")
    $proc = $launch4.Process
    $sheet4 = Wait-ForDialog -OwnerPid $proc.Id
    if (-not $sheet4) { throw 'no wizard on the fourth pass' }
    $h4 = $sheet4.Handle

    $p = Wait-ForPageControl -Sheet $h4 -CtrlId $IDC_S1_COMBO
    $combo4 = [NativeUi]::GetDlgItem($p, $IDC_S1_COMBO)
    [void][NativeUi]::SendMessageW($combo4, 0x014E, [IntPtr]$Language, [IntPtr]::Zero)
    Press-WizardButton -Sheet $h4 -Button $PSBTN_NEXT
    $p = Wait-ForPageControl -Sheet $h4 -CtrlId $IDC_S2_MOTD
    Press-WizardButton -Sheet $h4 -Button $PSBTN_NEXT
    $p = Wait-ForPageControl -Sheet $h4 -CtrlId $IDC_S3_PATH
    Set-CtrlText -Dialog $p -CtrlId $IDC_S3_PATH -Value $gameFolder
    Invoke-EditChanged -Dialog $p -CtrlId $IDC_S3_PATH
    Start-Sleep -Milliseconds 900
    Invoke-Button -Dialog $p -CtrlId $IDC_S3_RESTORE

    $rest = Wait-ForPageControl -Sheet $h4 -CtrlId $IDC_RESTORE_POINTS -TimeoutMs 15000
    Assert-That 'Restore/Uninstall opens the restore scene' ($rest -ne [IntPtr]::Zero)
    if ($rest -ne [IntPtr]::Zero) {
        Assert-Equal 'restore scene title' `
        (Get-CtrlText $rest $IDC_SCENE_TITLE) $S['IDS_RESTORE_TITLE']
        Assert-Equal 'primary button reads Restore' `
        (Get-WizardButtonText -Sheet $h4 -ButtonId 0x3024) $S['IDS_BTN_RESTORE']

        # Every run above left a point, plus the "remove everything" row.
        $rows = Get-ListCount -Dialog $rest -CtrlId $IDC_RESTORE_POINTS
        Assert-That 'the points taken by the earlier runs are listed' ($rows -ge 2) `
            "list holds $rows rows"
        Assert-That 'nothing is chosen until the user chooses' `
        ((Get-ListSelection -Dialog $rest -CtrlId $IDC_RESTORE_POINTS) -eq -1)

        # Restore with nothing picked must not do anything.
        Press-WizardButton -Sheet $h4 -Button $PSBTN_NEXT
        $warn = Wait-ForDialog -OwnerPid $proc.Id -NotHandle "$h4" -TimeoutMs 4000
        Assert-That 'Restore with nothing chosen asks for a choice' ($null -ne $warn)
        if ($warn) { Dismiss-Modal -Modal $warn.Handle }

        $pointsBefore = @(Get-ChildItem (Join-Path $gameFolder '.pacenotes\restore') `
                -ErrorAction SilentlyContinue).Count

        # The last row is "remove everything" — the oldest state there is.
        [void](Select-ListItem -Dialog $rest -CtrlId $IDC_RESTORE_POINTS -Index ($rows - 1))
        Start-Sleep -Milliseconds 400
        $preview = Get-CtrlText $rest $IDC_RESTORE_PREVIEW
        Assert-That 'the preview says what would be removed' `
        ($preview.Length -gt 0 -and $preview.Contains($S['IDS_RESTORE_REMOVE'].Split("`r")[0])) `
            "preview said [$preview]"
        # It names packages, not keys: a user reads leaves, not paths.
        Assert-That 'and names them' ($preview -match 'Kamil|Ewa|polish|Numeric') `
            "preview said [$preview]"

        Press-WizardButton -Sheet $h4 -Button $PSBTN_NEXT
        $done = Wait-ForPageControl -Sheet $h4 -CtrlId $IDC_S7_RESULT -TimeoutMs 40000
        Assert-That 'the restore runs and reaches the summary' ($done -ne [IntPtr]::Zero)
        if ($done -ne [IntPtr]::Zero) {
            Assert-That 'the summary says what it went back to' `
            ((Get-CtrlText $done $IDC_S7_RESULT).Length -gt 0)
            Assert-That 'the process survived it' (-not $proc.HasExited)
        }

        # And the folder really is back: our files gone, ledger empty.
        Assert-That 'the overlay is gone from the game folder' `
        (-not (Test-Path (Join-Path $gameFolder 'Plugins\Pacenote\config\x.ini')))
        $ledgerPath = Join-Path $gameFolder '.pacenotes\installed.json'
        # An empty ledger is `"packages": {}`, whose PSObject has no
        # properties at all — asking for .Name on that is an error under
        # StrictMode, not an empty list.
        $left = @()
        if (Test-Path $ledgerPath) {
            $packages = (Get-Content $ledgerPath -Raw | ConvertFrom-Json).packages
            if ($packages) { $left = @($packages.PSObject.Properties | ForEach-Object Name) }
        }
        Assert-That 'the ledger is empty afterwards' ($left.Count -eq 0) `
            "ledger still holds $($left -join ', ')"
        # A restore changes the folder like any other run, so it has to
        # leave a way back from ITSELF — a user who restores to the wrong
        # state must not be stuck there.
        $pointsAfter = @(Get-ChildItem (Join-Path $gameFolder '.pacenotes\restore') `
                -ErrorAction SilentlyContinue).Count
        Assert-That 'the restore is itself undoable' ($pointsAfter -eq $pointsBefore + 1) `
            "points went from $pointsBefore to $pointsAfter"
    }

    # ---------------------------------------------------------------
    # Fifth pass — /health fails, the catalogue answers (D12)
    # ---------------------------------------------------------------
    # One failed /health used to make the whole run offline, although
    # /packages would have answered: a load balancer hiccup, or a /health
    # slower than the rest. A stand-in in front of the real server fails
    # /health and relays everything else, which is that situation.
    Write-Host "`n-- fifth pass: /health fails, the catalogue answers --"
    $proc.Kill(); $proc.WaitForExit(5000) | Out-Null

    $front = Get-FreePort
    $standIn = [System.Net.HttpListener]::new()
    $standIn.Prefixes.Add("http://127.0.0.1:$front/")
    $standIn.Start()
    $relay = [powershell]::Create()
    [void]$relay.AddScript({
            param($listener, $behind)
            $client = [System.Net.Http.HttpClient]::new()
            while ($listener.IsListening) {
                try { $ctx = $listener.GetContext() } catch { break }
                if ($ctx.Request.Url.AbsolutePath -eq '/health') {
                    $ctx.Response.StatusCode = 503
                } else {
                    $ask = [System.Net.Http.HttpRequestMessage]::new(
                        [System.Net.Http.HttpMethod]::Get, "$behind$($ctx.Request.Url.PathAndQuery)")
                    $auth = $ctx.Request.Headers['Authorization']
                    if ($auth) { [void]$ask.Headers.TryAddWithoutValidation('Authorization', $auth) }
                    $answer = $client.SendAsync($ask).Result
                    $body = $answer.Content.ReadAsByteArrayAsync().Result
                    $ctx.Response.StatusCode = [int]$answer.StatusCode
                    $ctx.Response.ContentLength64 = $body.Length
                    $ctx.Response.OutputStream.Write($body, 0, $body.Length)
                }
                $ctx.Response.Close()
            }
        }).AddArgument($standIn).AddArgument("http://127.0.0.1:$port")
    $relayRun = $relay.BeginInvoke()
    try {
        $launch5 = Start-Installer -ExePath $ExePath -ExtraArgs @('--server', "http://127.0.0.1:$front")
        $proc = $launch5.Process
        $sheet5 = Wait-ForDialog -OwnerPid $proc.Id
        if (-not $sheet5) { throw 'no wizard on the fifth pass' }
        $h5 = $sheet5.Handle

        $p = Wait-ForPageControl -Sheet $h5 -CtrlId $IDC_S1_COMBO
        $combo5 = [NativeUi]::GetDlgItem($p, $IDC_S1_COMBO)
        [void][NativeUi]::SendMessageW($combo5, 0x014E, [IntPtr]$Language, [IntPtr]::Zero)
        Press-WizardButton -Sheet $h5 -Button $PSBTN_NEXT
        $p = Wait-ForPageControl -Sheet $h5 -CtrlId $IDC_S2_MOTD
        $deadline = (Get-Date).AddSeconds(15)
        do {
            Start-Sleep -Milliseconds 300
            $status5 = Get-CtrlText $p $IDC_S2_STATUS
        } while ($status5 -cne $S['IDS_S2_OFFLINE'] -and (Get-Date) -lt $deadline)
        Assert-Equal 'scene 2 still reports what /health said' $status5 $S['IDS_S2_OFFLINE']
        Press-WizardButton -Sheet $h5 -Button $PSBTN_NEXT

        $p = Wait-ForPageControl -Sheet $h5 -CtrlId $IDC_S3_PATH
        Set-CtrlText -Dialog $p -CtrlId $IDC_S3_PATH -Value $gameFolder
        Invoke-EditChanged -Dialog $p -CtrlId $IDC_S3_PATH
        Start-Sleep -Milliseconds 900
        Invoke-Button -Dialog $p -CtrlId $IDC_S3_DOWNLOAD

        $p = Wait-ForPageControl -Sheet $h5 -CtrlId $IDC_S4_TREE
        Assert-That 'scene 4 opens' ($p -ne [IntPtr]::Zero)
        if ($p -ne [IntPtr]::Zero) {
            $tv5 = [NativeUi]::GetDlgItem($p, $IDC_S4_TREE)
            $deadline = (Get-Date).AddSeconds(20)
            do {
                Start-Sleep -Milliseconds 300
                $rows5 = [int][NativeUi]::SendMessageW($tv5, $TVM_GETCOUNT, [IntPtr]::Zero, [IntPtr]::Zero)
                $s4 = Get-CtrlText $p $IDC_S4_STATUS
            } while ($s4 -ceq $S['IDS_S4_LOADING'] -and (Get-Date) -lt $deadline)
            Assert-That 'the catalogue is fetched anyway' ($rows5 -eq $EXPECTED_ROWS) `
                "control holds $rows5 rows, expected $EXPECTED_ROWS"
            Assert-Equal 'and scene 4 says it is ready, not offline' $s4 $S['IDS_S4_READY']
        }
        $newest = Get-ChildItem -Path (Join-Path $gameFolder '.pacenotes\logs') -Filter 'install-*.log' |
            Sort-Object Name | Select-Object -Last 1
        $said = if ($newest) { Get-Content $newest.FullName -Raw } else { '' }
        Assert-That 'the run log records that /health and the catalogue disagreed' `
        ($said.Contains('the catalogue answered although /health had not')) "log said [$said]"
    } finally {
        $standIn.Close()
        $relay.Stop(); $relay.Dispose()
    }

    # ---------------------------------------------------------------
    # Sixth pass — one download fails once, another for good (§7)
    # ---------------------------------------------------------------
    # One failed package used to end the whole run. Now a transient failure
    # is retried, a lasting one is left out, the rest carry on, and scene 7
    # names what never arrived. A fresh game folder, so everything downloads.
    Write-Host "`n-- sixth pass: a download fails once, another for good --"
    $proc.Kill(); $proc.WaitForExit(5000) | Out-Null
    $gameFolder6 = Join-Path $work 'game6'
    [void](New-Item -ItemType Directory -Force -Path $gameFolder6)
    [void](New-Item -ItemType File -Path (Join-Path $gameFolder6 'RichardBurnsRally_SSE.exe'))

    $front6 = Get-FreePort
    $standIn6 = [System.Net.HttpListener]::new()
    $standIn6.Prefixes.Add("http://127.0.0.1:$front6/")
    $standIn6.Start()
    $seen6 = [hashtable]::Synchronized(@{ Pacenotes = 0; Kamil = 0 })
    $relay6 = [powershell]::Create()
    [void]$relay6.AddScript({
            param($listener, $behind, $seen)
            $client = [System.Net.Http.HttpClient]::new()
            while ($listener.IsListening) {
                try { $ctx = $listener.GetContext() } catch { break }
                try {
                    $path = $ctx.Request.Url.AbsolutePath
                    if ($path -eq '/download/rbr/public/codriver/polish/male/Kamil') {
                        $seen.Kamil++
                        $ctx.Response.StatusCode = 404
                    } elseif ($path -eq '/download/rbr/public/pacenotes/RBR' -and $seen.Pacenotes++ -eq 0) {
                        $ctx.Response.StatusCode = 503
                    } else {
                        $ask = [System.Net.Http.HttpRequestMessage]::new(
                            [System.Net.Http.HttpMethod]::Get, "$behind$($ctx.Request.Url.PathAndQuery)")
                        $answer = $client.SendAsync($ask).Result
                        $body = $answer.Content.ReadAsByteArrayAsync().Result
                        $ctx.Response.StatusCode = [int]$answer.StatusCode
                        $ctx.Response.ContentLength64 = $body.Length
                        $ctx.Response.OutputStream.Write($body, 0, $body.Length)
                    }
                    $ctx.Response.Close()
                } catch {
                    $ctx.Response.Abort()
                }
            }
        }).AddArgument($standIn6).AddArgument("http://127.0.0.1:$port").AddArgument($seen6)
    $relay6Run = $relay6.BeginInvoke()
    try {
        $launch6 = Start-Installer -ExePath $ExePath -ExtraArgs @('--server', "http://127.0.0.1:$front6")
        $proc = $launch6.Process
        $sheet6 = Wait-ForDialog -OwnerPid $proc.Id
        if (-not $sheet6) { throw 'no wizard on the sixth pass' }
        $h6 = $sheet6.Handle

        $p = Wait-ForPageControl -Sheet $h6 -CtrlId $IDC_S1_COMBO
        $combo6 = [NativeUi]::GetDlgItem($p, $IDC_S1_COMBO)
        [void][NativeUi]::SendMessageW($combo6, 0x014E, [IntPtr]$Language, [IntPtr]::Zero)
        Press-WizardButton -Sheet $h6 -Button $PSBTN_NEXT
        $p = Wait-ForPageControl -Sheet $h6 -CtrlId $IDC_S2_MOTD
        Press-WizardButton -Sheet $h6 -Button $PSBTN_NEXT
        $p = Wait-ForPageControl -Sheet $h6 -CtrlId $IDC_S3_PATH
        Set-CtrlText -Dialog $p -CtrlId $IDC_S3_PATH -Value $gameFolder6
        Invoke-EditChanged -Dialog $p -CtrlId $IDC_S3_PATH
        Start-Sleep -Milliseconds 900
        Invoke-Button -Dialog $p -CtrlId $IDC_S3_DOWNLOAD

        $p = Wait-ForPageControl -Sheet $h6 -CtrlId $IDC_S4_TREE
        $tv6 = [NativeUi]::GetDlgItem($p, $IDC_S4_TREE)
        $deadline = (Get-Date).AddSeconds(20)
        do {
            Start-Sleep -Milliseconds 300
            $rows6 = [int][NativeUi]::SendMessageW($tv6, $TVM_GETCOUNT, [IntPtr]::Zero, [IntPtr]::Zero)
        } while ($rows6 -eq 0 -and (Get-Date) -lt $deadline)
        [void](Invoke-TreeCheckRoots -Dialog $p -CtrlId $IDC_S4_TREE)
        Press-WizardButton -Sheet $h6 -Button $PSBTN_NEXT

        $p5 = Wait-ForPageControl -Sheet $h6 -CtrlId $IDC_S5_LANG -TimeoutMs 40000
        Assert-That 'one failed download no longer ends the run' ($p5 -ne [IntPtr]::Zero)
        if ($p5 -eq [IntPtr]::Zero) { throw 'scene 5 never appeared on the sixth pass' }
        Assert-That 'the busy one was asked for again' ($seen6.Pacenotes -eq 2) "asked $($seen6.Pacenotes) times"
        Assert-That 'the missing one was asked for once, since a 404 will not change' `
        ($seen6.Kamil -eq 1) "asked $($seen6.Kamil) times"
        $voices = Get-ListCount -Dialog $p5 -CtrlId $IDC_S5_VOICE
        Assert-That 'the voice that never arrived is not offered' ($voices -eq 2) "list holds $voices"

        foreach ($id in $IDC_S5_LANG, $IDC_S5_VOICE, $IDC_S5_STYLE) {
            [void](Select-ListItem -Dialog $p5 -CtrlId $id -Index 0)
        }
        Press-WizardButton -Sheet $h6 -Button $PSBTN_NEXT
        $p7 = Wait-ForPageControl -Sheet $h6 -CtrlId $IDC_S7_RESULT -TimeoutMs 40000
        Assert-That 'the rest installs and reaches the summary' ($p7 -ne [IntPtr]::Zero)
        if ($p7 -ne [IntPtr]::Zero) {
            $detail6 = Get-CtrlText $p7 $IDC_S7_DETAIL
            $named = Expand-ResourceArg -Template $S['IDS_S7_NOT_DOWNLOADED'] -Value 'Kamil'
            Assert-That 'scene 7 names what could not be downloaded' ($detail6.Contains($named)) `
                "summary said [$detail6]"
            Assert-That 'and opens the log, since something went wrong' `
            (Test-CtrlVisible -Dialog $p7 -CtrlId $IDC_S7_LOG)
        }
        $ledger6 = Join-Path $gameFolder6 '.pacenotes\installed.json'
        $installed6 = if (Test-Path $ledger6) {
            @((Get-Content $ledger6 -Raw | ConvertFrom-Json).packages.PSObject.Properties.Name)
        } else { @() }
        Assert-That 'the package that arrived on its retry was installed' `
        ($installed6 -contains 'rbr/public/pacenotes/RBR') "ledger holds $($installed6 -join ', ')"
        Assert-That 'and the one that never arrived was not' `
        ($installed6 -notcontains 'rbr/public/codriver/polish/male/Kamil')
    } finally {
        $standIn6.Close()
        $relay6.Stop(); $relay6.Dispose()
    }

    # ---------------------------------------------------------------
    # Seventh pass — the game is running (P12)
    # ---------------------------------------------------------------
    # A running game holds its files open, so an install would fail partway.
    # The "game" is Windows' own ping.exe under the marker's name, which just
    # waits; first from another folder, which must not block this one, then
    # from this one, which must.
    Write-Host "`n-- seventh pass: the game is running --"
    $proc.Kill(); $proc.WaitForExit(5000) | Out-Null
    $marker = 'RichardBurnsRally_SSE.exe'
    $elsewhere = Join-Path $work 'other-install'
    [void](New-Item -ItemType Directory -Force -Path $elsewhere)
    $fakeGames = @()

    <#  Configure/Install on $gameFolder with the first option of each list;
        returns scene 7's result line. #>
    function Invoke-ConfigureRun {
        $launch7 = Start-Installer -ExePath $ExePath -ExtraArgs @('--server', "http://127.0.0.1:$port")
        $script:proc = $launch7.Process
        $h7 = (Wait-ForDialog -OwnerPid $script:proc.Id).Handle
        $p = Wait-ForPageControl -Sheet $h7 -CtrlId $IDC_S1_COMBO
        [void][NativeUi]::SendMessageW([NativeUi]::GetDlgItem($p, $IDC_S1_COMBO), 0x014E, [IntPtr]$Language, [IntPtr]::Zero)
        Press-WizardButton -Sheet $h7 -Button $PSBTN_NEXT
        [void](Wait-ForPageControl -Sheet $h7 -CtrlId $IDC_S2_MOTD)
        Press-WizardButton -Sheet $h7 -Button $PSBTN_NEXT
        $p = Wait-ForPageControl -Sheet $h7 -CtrlId $IDC_S3_PATH
        Set-CtrlText -Dialog $p -CtrlId $IDC_S3_PATH -Value $gameFolder
        Invoke-EditChanged -Dialog $p -CtrlId $IDC_S3_PATH
        Start-Sleep -Milliseconds 900
        Invoke-Button -Dialog $p -CtrlId $IDC_S3_CONFIGURE
        $p5 = Wait-ForPageControl -Sheet $h7 -CtrlId $IDC_S5_LANG -TimeoutMs 10000
        foreach ($id in $IDC_S5_LANG, $IDC_S5_VOICE, $IDC_S5_STYLE) {
            [void](Select-ListItem -Dialog $p5 -CtrlId $id -Index 0)
        }
        Press-WizardButton -Sheet $h7 -Button $PSBTN_NEXT
        $p7 = Wait-ForPageControl -Sheet $h7 -CtrlId $IDC_S7_RESULT -TimeoutMs 40000
        $result = if ($p7 -ne [IntPtr]::Zero) { Get-CtrlText $p7 $IDC_S7_RESULT } else { '' }
        $script:proc.Kill(); $script:proc.WaitForExit(5000) | Out-Null
        return $result
    }
    function Start-FakeGame([string]$Folder) {
        $exe = Join-Path $Folder $marker
        Copy-Item (Join-Path $env:SystemRoot 'System32\PING.EXE') $exe -Force
        return Start-Process $exe -PassThru -WindowStyle Hidden -ArgumentList @('-n', '300', '127.0.0.1')
    }
    try {
        $fakeGames += Start-FakeGame $elsewhere
        $result7 = Invoke-ConfigureRun
        Assert-Equal 'a copy of the game running from another folder does not block this one' `
            $result7 $S['IDS_S7_OK']
        $fakeGames | ForEach-Object { if (-not $_.HasExited) { $_.Kill(); $_.WaitForExit(5000) | Out-Null } }

        $ledgerPath = Join-Path $gameFolder '.pacenotes\installed.json'
        $ledgerBefore = Get-Content -Raw $ledgerPath
        $fakeGames += Start-FakeGame $gameFolder
        $result7 = Invoke-ConfigureRun
        $failedPrefix = $S['IDS_S7_FAILED'].Substring(0, $S['IDS_S7_FAILED'].IndexOf('%s'))
        Assert-That 'the game running from this folder stops the install' `
        ($result7.StartsWith($failedPrefix) -and $result7.Contains('the game is running')) "result was [$result7]"
        Assert-That 'before anything in the folder changed' ((Get-Content -Raw $ledgerPath) -eq $ledgerBefore)
    } finally {
        $fakeGames | ForEach-Object { if (-not $_.HasExited) { $_.Kill(); $_.WaitForExit(5000) | Out-Null } }
    }

    # ---------------------------------------------------------------
    # Eighth pass — a category with nothing to pick from after 4b
    # ---------------------------------------------------------------
    # Every co-driver download fails for good, so scene 5 would ask for a
    # voice with none to offer. The transfer goes back to scene 4 instead,
    # with the voices unticked there too, and a new selection fetches again.
    Write-Host "`n-- eighth pass: no voice arrives --"
    $gameFolder8 = Join-Path $work 'game8'
    [void](New-Item -ItemType Directory -Force -Path $gameFolder8)
    [void](New-Item -ItemType File -Path (Join-Path $gameFolder8 'RichardBurnsRally_SSE.exe'))

    $front8 = Get-FreePort
    $standIn8 = [System.Net.HttpListener]::new()
    $standIn8.Prefixes.Add("http://127.0.0.1:$front8/")
    $standIn8.Start()
    $seen8 = [hashtable]::Synchronized(@{ Voices = 0 })
    $relay8 = [powershell]::Create()
    [void]$relay8.AddScript({
            param($listener, $behind, $seen)
            $client = [System.Net.Http.HttpClient]::new()
            while ($listener.IsListening) {
                try { $ctx = $listener.GetContext() } catch { break }
                try {
                    if ($ctx.Request.Url.AbsolutePath.StartsWith('/download/rbr/public/codriver/')) {
                        $seen.Voices++
                        $ctx.Response.StatusCode = 404
                    } else {
                        $ask = [System.Net.Http.HttpRequestMessage]::new(
                            [System.Net.Http.HttpMethod]::Get, "$behind$($ctx.Request.Url.PathAndQuery)")
                        $answer = $client.SendAsync($ask).Result
                        $body = $answer.Content.ReadAsByteArrayAsync().Result
                        $ctx.Response.StatusCode = [int]$answer.StatusCode
                        $ctx.Response.ContentLength64 = $body.Length
                        $ctx.Response.OutputStream.Write($body, 0, $body.Length)
                    }
                    $ctx.Response.Close()
                } catch {
                    $ctx.Response.Abort()
                }
            }
        }).AddArgument($standIn8).AddArgument("http://127.0.0.1:$port").AddArgument($seen8)
    $relay8Run = $relay8.BeginInvoke()
    try {
        $launch8 = Start-Installer -ExePath $ExePath -ExtraArgs @('--server', "http://127.0.0.1:$front8")
        $proc = $launch8.Process
        $sheet8 = Wait-ForDialog -OwnerPid $proc.Id
        if (-not $sheet8) { throw 'no wizard on the eighth pass' }
        $h8 = $sheet8.Handle

        $p = Wait-ForPageControl -Sheet $h8 -CtrlId $IDC_S1_COMBO
        [void][NativeUi]::SendMessageW([NativeUi]::GetDlgItem($p, $IDC_S1_COMBO), 0x014E, [IntPtr]$Language, [IntPtr]::Zero)
        Press-WizardButton -Sheet $h8 -Button $PSBTN_NEXT
        [void](Wait-ForPageControl -Sheet $h8 -CtrlId $IDC_S2_MOTD)
        Press-WizardButton -Sheet $h8 -Button $PSBTN_NEXT
        $p = Wait-ForPageControl -Sheet $h8 -CtrlId $IDC_S3_PATH
        Set-CtrlText -Dialog $p -CtrlId $IDC_S3_PATH -Value $gameFolder8
        Invoke-EditChanged -Dialog $p -CtrlId $IDC_S3_PATH
        Start-Sleep -Milliseconds 900
        Invoke-Button -Dialog $p -CtrlId $IDC_S3_DOWNLOAD

        $p4 = Wait-ForPageControl -Sheet $h8 -CtrlId $IDC_S4_TREE
        $deadline = (Get-Date).AddSeconds(20)
        do {
            Start-Sleep -Milliseconds 300
        } while ((Get-TreeRowCount -Dialog $p4 -CtrlId $IDC_S4_TREE) -eq 0 -and (Get-Date) -lt $deadline)

        [void](Invoke-TreeCheckRoots -Dialog $p4 -CtrlId $IDC_S4_TREE)
        Press-WizardButton -Sheet $h8 -Button $PSBTN_NEXT
        $warn8 = Wait-ForDialog -OwnerPid $proc.Id -NotHandle "$h8" -TimeoutMs 40000
        Assert-That 'no voice arriving is named rather than asked about' ($null -ne $warn8)
        if (-not $warn8) { throw 'no warning after the voices failed' }
        $said = (Get-ControlInventory -Dialog $warn8.Handle |
            Where-Object { $_.Class -eq 'Static' } | ForEach-Object { $_.Text }) -join ' '
        $voice = $S['IDS_S5_VOICE'].TrimEnd(':')
        Assert-That "the warning names '$voice' alone" `
        ($said.Contains($notReady) -and $said.Contains($voice) -and
            -not $said.Contains($S['IDS_S5_LANGUAGE'].TrimEnd(':'))) "warning said [$said]"
        Dismiss-Modal -Modal $warn8.Handle

        $p4 = Wait-ForPageControl -Sheet $h8 -CtrlId $IDC_S4_TREE -TimeoutMs 5000
        Assert-That 'it is back to scene 4, not on to the questions' ($p4 -ne [IntPtr]::Zero)
        if ($p4 -eq [IntPtr]::Zero) { throw 'scene 4 did not come back' }
        $tv8 = [NativeUi]::GetDlgItem($p4, $IDC_S4_TREE)
        $root = [NativeUi]::SendMessageW($tv8, $TVM_GETNEXTITEM, [IntPtr]$TVGN_ROOT, [IntPtr]::Zero)
        $images = @()
        while ($root -ne [IntPtr]::Zero) {
            $images += Get-TreeStateImage -Tree $tv8 -Item $root
            $root = [NativeUi]::SendMessageW($tv8, $TVM_GETNEXTITEM, [IntPtr]$TVGN_NEXT, $root)
        }
        Assert-That 'the voices are unticked there, the rest stay ticked' `
        ((@($images | Where-Object { $_ -eq 1 }).Count -eq 1) -and (@($images | Where-Object { $_ -eq 2 }).Count -eq 3)) `
            "root state images: $($images -join ', ')"

        # Only the voices are missing now, so this transfer brings nothing at
        # all, and that still ends at the summary: a server that delivers
        # nothing is what the run log is there to explain.
        $before = $seen8.Voices
        [void](Invoke-TreeCheckRoots -Dialog $p4 -CtrlId $IDC_S4_TREE)
        Press-WizardButton -Sheet $h8 -Button $PSBTN_NEXT
        $p7 = Wait-ForPageControl -Sheet $h8 -CtrlId $IDC_S7_RESULT -TimeoutMs 40000
        Assert-That 'a new selection runs a transfer of its own' `
        ($seen8.Voices -gt $before) "voices asked for: $before before, $($seen8.Voices) after"
        $result8 = if ($p7 -ne [IntPtr]::Zero) { Get-CtrlText $p7 $IDC_S7_RESULT } else { '' }        Assert-That 'and when it brings nothing, the run ends at the summary' `
        ($result8.Contains($S['IDS_S7_NONE_DOWNLOADED'])) "result was [$result8]"
    } finally {
        $standIn8.Close()
        $relay8.Stop(); $relay8.Dispose()
    }
} finally {
    if ($proc -and -not $proc.HasExited) { $proc.Kill(); $proc.WaitForExit(2000) | Out-Null }
    if ($srvProc -and -not $srvProc.HasExited) { Stop-Process -Id $srvProc.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}

Write-Host "`n=== $($script:checks - $script:failures.Count)/$($script:checks) checks passed ===" -ForegroundColor Cyan
if ($script:failures.Count -gt 0) {
    Write-Host 'FAILED:' -ForegroundColor Red
    $script:failures | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    exit 1
}
Write-Host 'Download scene works against a live catalogue.' -ForegroundColor Green
exit 0
