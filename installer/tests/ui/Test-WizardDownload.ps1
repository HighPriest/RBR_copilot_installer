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
$TVM_GETCOUNT = 0x1105

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
