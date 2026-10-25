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
$IDC_S4_TREE = 331; $IDC_S4_STATUS = 332
$IDC_S5_LANG_LABEL = 341; $IDC_S5_LANG = 342
$IDC_S5_VOICE_LABEL = 343; $IDC_S5_VOICE = 344
$IDC_S5_STYLE_LABEL = 345; $IDC_S5_STYLE = 346
$IDC_S7_RESULT = 361; $IDC_S7_DETAIL = 362; $IDC_S7_LOG = 363
$PSBTN_NEXT = 1
$TVM_GETCOUNT = 0x1105

$crateRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$repoRoot = Split-Path -Parent $crateRoot
$server = Join-Path $repoRoot 'target\debug\pacenotes-server.exe'
$S = Get-ResourceStrings -Language $(if ($Language -eq 0) { 'en' } else { 'pl' })

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

    # A cache of its own, so the run downloads for real and never touches the user's cache.
    $cacheDir = Join-Path $work 'cache'
    $launch = Start-Installer -ExePath $ExePath -ExtraArgs @(
        '--server', "http://127.0.0.1:$port", '--cache', $cacheDir)
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
    Press-WizardButton -Sheet $h -Button $PSBTN_NEXT

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

    Write-Host "`n-- download gate --"
    Press-WizardButton -Sheet $h -Button $PSBTN_NEXT
    $modal = Wait-ForDialog -OwnerPid $proc.Id -NotHandle "$h" -TimeoutMs 4000
    Assert-That 'Download with nothing ticked raises a warning' ($null -ne $modal)
    if ($modal) {
        # The warning must NAME the categories, or the user cannot act on it.
        $said = (Get-ControlInventory -Dialog $modal.Handle |
            Where-Object { $_.Class -eq 'Static' } | ForEach-Object { $_.Text }) -join ' '
        foreach ($category in 'language', 'codriver', 'style', 'pacenotes') {
            Assert-That "warning names the missing category '$category'" `
            ($said -clike "*$category*") "warning said [$said]"
        }
        Dismiss-Modal -Modal $modal.Handle
    }
    $still = Wait-ForPageControl -Sheet $h -CtrlId $IDC_S4_TREE -TimeoutMs 3000
    Assert-That 'and the wizard stays on scene 4' ($still -ne [IntPtr]::Zero)

    # Ticking every category is what the gate asked for.
    $ticked = Invoke-TreeCheckRoots -Dialog $page -CtrlId $IDC_S4_TREE
    Assert-That 'every category can be ticked' ($ticked -eq 4) "ticked $ticked roots, expected 4"
    Press-WizardButton -Sheet $h -Button $PSBTN_NEXT

    Write-Host "`n-- scene 5: configuration --"
    $page5 = Wait-ForPageControl -Sheet $h -CtrlId $IDC_S5_LANG
    Assert-That 'a tick in every category gets past the gate' ($page5 -ne [IntPtr]::Zero)
    if ($page5 -eq [IntPtr]::Zero) { throw 'scene 5 never appeared' }

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
