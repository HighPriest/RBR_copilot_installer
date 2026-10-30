<#
.SYNOPSIS
    AUTOMATED TEST. Walks the wizard to scene 4 against a REAL server holding
    a nested catalogue, and asserts the Download scene. Exits 0/1.

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
    & $server manifest gen --data $data 2>&1 | Out-Null

    $port = Get-FreePort
    $srvProc = Start-Process $server -PassThru -WindowStyle Hidden -ArgumentList @(
        'serve', '--listen', "127.0.0.1:$port", '--data', $data, '--db-url', "file:$(Join-Path $work 'auth.db')")

    # A folder that really is an RBR install, so scene 3 lets us through.
    $gameFolder = Join-Path $work 'game'
    [void](New-Item -ItemType Directory -Force -Path $gameFolder)
    [void](New-Item -ItemType File -Path (Join-Path $gameFolder 'RichardBurnsRally_SSE.exe'))

    $launch = Start-Installer -ExePath $ExePath -ExtraArgs @('--server', "http://127.0.0.1:$port")
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
