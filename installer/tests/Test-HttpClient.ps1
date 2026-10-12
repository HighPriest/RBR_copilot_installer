<#
.SYNOPSIS
    AUTOMATED TEST. Drives the installer's WinHTTP client against a REAL
    pacenotes-server and asserts what comes back. Exits 0 on success, 1 on any
    failed assertion.

.DESCRIPTION
    `src/http.rs` is Win32 all the way down, so it cannot be unit tested. What
    can be tested is the thing it exists for: fetching from the actual server
    this installer talks to. So this builds the server, publishes a real
    overlay zip through `manifest gen`, mints a real credential, starts the
    server on a loopback port, and runs the installer's `--selftest-http`
    against it.

    No mock, and no arbitrary inputs: every URL here is one the installer will
    really request, and the expected bytes come from the manifest the server
    generated rather than from literals typed into this file.

    Covers the four outcomes the UI has to tell apart (installer/CLAUDE.md
    §Error handling, and task I8's access-key UX):
      200 with a body     — the happy path, Content-Length honoured
      404                 — package genuinely absent
      401 / 403           — key missing vs key lacking the grant
      transport failure   — server unreachable, reported in words not a code

    Needs Windows (the installer is Win32) and a cargo toolchain. Does NOT
    need a desktop session: --selftest-http writes to a file and never opens a
    window, which is why this one CAN run in CI.

.PARAMETER Port
    Loopback port for the test server. Defaults to one the OS says is free:
    a fixed port collides with a previous run whose server outlived it, and
    with whatever else happens to be listening on a CI runner.

.EXAMPLE
    pwsh -File tests/Test-HttpClient.ps1
#>
[CmdletBinding()]
param([int]$Port = 0)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$crateRoot = Split-Path -Parent $PSScriptRoot
$repoRoot = Split-Path -Parent $crateRoot
$exe = Join-Path $crateRoot 'target\x86_64-pc-windows-msvc\release\pacenotes-installer.exe'
$server = Join-Path $repoRoot 'target\debug\pacenotes-server.exe'

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

<#  Run one --selftest-http and return the parsed report plus the exit code.
    The report is deliberately a flat KEY: VALUE format so a test can assert
    on fields instead of pattern-matching prose. #>
function Invoke-SelfTest {
    param([Parameter(Mandatory)][string]$Url, [string]$Token)
    $out = Join-Path $script:work ("report-" + [guid]::NewGuid() + ".txt")
    # Not $args: that is a PowerShell automatic variable.
    $cliArgs = @('--selftest-http', $Url, '--out', $out)
    if ($Token) { $cliArgs += @('--token', $Token) }
    $proc = Start-Process $exe -ArgumentList $cliArgs -PassThru -Wait
    $report = @{ ExitCode = $proc.ExitCode }
    foreach ($line in Get-Content $out) {
        $m = [regex]::Match($line, '^(?<k>[A-Z_]+): ?(?<v>.*)$')
        if ($m.Success) { $report[$m.Groups['k'].Value] = $m.Groups['v'].Value }
    }
    return $report
}

<#  Ask the OS for a free loopback port by binding one and letting go. There is
    a small race before the server claims it, but far smaller than the chance
    of colliding with a fixed port on a shared CI runner. #>
function Get-FreePort {
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $port = $listener.LocalEndpoint.Port
    $listener.Stop()
    return $port
}

Write-Host '=== installer HTTP client vs live pacenotes-server ===' -ForegroundColor Cyan
if ($Port -eq 0) { $Port = Get-FreePort }
Write-Host "test server port: $Port"

if (-not (Test-Path $exe)) {
    Write-Error "Installer not built. Run: cargo build --release --target x86_64-pc-windows-msvc"
    exit 1
}

Write-Host 'building pacenotes-server...'
Push-Location $repoRoot
# The server is a different crate with its own CI job and its own lint policy.
# Inheriting this crate's RUSTFLAGS would both impose our policy on it and
# force a full rebuild of its dependency tree whenever the flags differ.
$savedRustflags = $env:RUSTFLAGS
$env:RUSTFLAGS = $null
try {
    cargo build -p pacenotes-server 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Error 'server build failed; run `cargo build -p pacenotes-server` to see why'
        exit 1
    }
} finally {
    $env:RUSTFLAGS = $savedRustflags
    Pop-Location
}

$script:work = Join-Path ([System.IO.Path]::GetTempPath()) ("pacenotes-http-" + [guid]::NewGuid())
# One server serves every game: <data>/<game>/packages/<directory>/<name>.zip
$dataRoot = Join-Path $work 'data'
$proc = $null
try {
    # ---- Publish real overlay zips -----------------------------------------
    # A game-root overlay: extracting at the game dir puts the file in place.
    # The same directory+name is published under TWO games on purpose: that
    # pair is no longer unique, and every URL and key has to carry the game.
    $stage = Join-Path $work 'stage\Plugins\Pacenote\config'
    [void](New-Item -ItemType Directory -Force -Path $stage)
    Set-Content -Path (Join-Path $stage 'Voice.ini') -Value "; test overlay`r`nname=TestVoice"
    foreach ($game in 'rbr', 'dirtrally2') {
        foreach ($dir in 'public', 'premium-voices') {
            $into = Join-Path $dataRoot "$game\packages\$dir\codriver\polish\male"
            [void](New-Item -ItemType Directory -Force -Path $into)
            Compress-Archive -Path (Join-Path $work 'stage\Plugins') `
                -DestinationPath (Join-Path $into 'Kamil.zip') -Force
        }
    }

    & $server manifest gen --data $dataRoot | Out-Null
    $manifest = Get-Content -Raw (Join-Path $dataRoot 'manifest.json') | ConvertFrom-Json
    Assert-That 'server published all four packages' ($manifest.packages.Count -eq 4) `
        "manifest lists $($manifest.packages.Count)"
    $public = $manifest.packages | Where-Object { $_.directory -eq 'public' -and $_.game -eq 'rbr' }
    $protected = $manifest.packages | Where-Object { $_.directory -eq 'premium-voices' -and $_.game -eq 'rbr' }
    # The nested folders became the package's name, which is the UI tree.
    Assert-That 'a package name is its path under the access directory' `
    ($public.name -eq 'codriver/polish/male/Kamil') "name is $($public.name)"

    # ---- Mint a credential granted only the protected directory ------------
    # --db-url belongs to `token`, not to `mint`.
    $db = "file:$(Join-Path $work 'auth.db')"
    $mint = (& $server token --db-url $db mint --label 'http-client-test' --grant 'premium-voices') -join "`n"
    $tokenMatch = [regex]::Match($mint, 'pcn_[A-Za-z0-9_\-]+')
    Assert-That 'server minted a credential' $tokenMatch.Success "mint said: $mint"
    if (-not $tokenMatch.Success) { throw 'cannot continue without a credential' }
    $token = $tokenMatch.Value

    # ---- Start it ----------------------------------------------------------
    $proc = Start-Process $server -PassThru -WindowStyle Hidden -ArgumentList @(
        'serve', '--listen', "127.0.0.1:$Port", '--data', $dataRoot, '--db-url', $db)
    $base = "http://127.0.0.1:$Port"
    $deadline = (Get-Date).AddSeconds(15)
    do {
        Start-Sleep -Milliseconds 200
        $up = (Invoke-SelfTest -Url "$base/health").RESULT -eq 'OK'
    } while (-not $up -and (Get-Date) -lt $deadline)
    Assert-That 'server answers /health' $up

    # ---- 200 with a body ---------------------------------------------------
    Write-Host "`n-- GET /health --"
    $r = Invoke-SelfTest -Url "$base/health"
    Assert-That 'health succeeds' ($r.RESULT -eq 'OK') "report says $($r.RESULT)"
    Assert-That 'health exits 0' ($r.ExitCode -eq 0) "exit $($r.ExitCode)"
    Assert-That "health body is the server's 'ok'" ($r.BODY -eq 'ok') "got [$($r.BODY)]"
    Assert-That 'Content-Length matches the bytes read' ($r.CONTENT_LENGTH -eq $r.BYTES) `
        "declared $($r.CONTENT_LENGTH), read $($r.BYTES)"

    # ---- The manifest fetch M4 asks for -----------------------------------
    Write-Host "`n-- GET /packages (anonymous) --"
    $r = Invoke-SelfTest -Url "$base/packages?game=rbr"
    Assert-That 'packages succeeds' ($r.RESULT -eq 'OK')
    Assert-That 'anonymous listing names the public package' `
    ($r.BODY.Contains("`"name`":`"$($public.name)`"")) "body was [$($r.BODY)]"
    Assert-That 'anonymous listing hides the protected directory' `
    (-not $r.BODY.Contains('premium-voices')) "body was [$($r.BODY)]"
    # The sha is what the installer will verify a download against, so the
    # bytes must survive the transport exactly.
    Assert-That 'listing carries the sha the server computed' `
    ($r.BODY.Contains($public.sha256))
    # One server holds every game, so the filter has to be real: the other
    # game publishes the SAME directory+name, and it must not appear here.
    Assert-That '?game= filters other games out server-side' `
    (-not $r.BODY.Contains('dirtrally2')) "body was [$($r.BODY)]"

    Write-Host "`n-- GET /packages (with access key) --"
    $r = Invoke-SelfTest -Url "$base/packages?game=rbr" -Token $token
    Assert-That 'authenticated listing succeeds' ($r.RESULT -eq 'OK')
    Assert-That 'granted directory now appears' ($r.BODY.Contains('premium-voices')) `
        "body was [$($r.BODY)]"
    Assert-That 'bearer header reached the server (listing grew)' `
    ($r.BODY.Contains($protected.sha256))

    # ---- Downloads: the bytes, and the auth outcomes -----------------------
    Write-Host "`n-- GET /download --"
    $r = Invoke-SelfTest -Url "$base/download/$($public.game)/$($public.directory)/$($public.name)"
    Assert-That 'public download succeeds' ($r.RESULT -eq 'OK')
    Assert-That 'download returns the whole zip' ([int]$r.BYTES -eq [int]$public.size) `
        "manifest says $($public.size), read $($r.BYTES)"

    $r = Invoke-SelfTest -Url "$base/download/$($protected.game)/$($protected.directory)/$($protected.name)"
    Assert-That 'protected download without a key is refused' ($r.RESULT -eq 'FAIL')
    Assert-That 'refusal is 401, not a transport error' ($r.STATUS -eq '401') `
        "status $($r.STATUS), message $($r.MESSAGE)"

    $r = Invoke-SelfTest -Url "$base/download/$($protected.game)/$($protected.directory)/$($protected.name)" -Token $token
    Assert-That 'protected download with the granted key succeeds' ($r.RESULT -eq 'OK') `
        "message $(if ($r.ContainsKey('MESSAGE')) { $r.MESSAGE } else { '' })"
    Assert-That 'protected download returns the whole zip' `
    ([int]$r.BYTES -eq [int]$protected.size) "manifest says $($protected.size), read $($r.BYTES)"

    # ---- 404 must be distinguishable from a dead server --------------------
    Write-Host "`n-- error surfaces --"
    $r = Invoke-SelfTest -Url "$base/download/rbr/public/no-such-package"
    Assert-That 'missing package fails' ($r.RESULT -eq 'FAIL')
    Assert-That 'missing package reports 404' ($r.STATUS -eq '404') "status $($r.STATUS)"
    Assert-That 'missing package exits non-zero' ($r.ExitCode -ne 0)

    # The game segment is load-bearing, not decoration: a real directory+name
    # under a game that does not have it must 404 rather than serve the other
    # game's bytes. This is why cache and ledger keys carry the game too.
    $r = Invoke-SelfTest -Url "$base/download/beamng/$($public.directory)/$($public.name)"
    Assert-That 'the same directory+name under another game is 404' `
    ($r.STATUS -eq '404') "status $($r.STATUS), message $($r.MESSAGE)"

    # Port 1 has nothing listening: a transport failure, with no HTTP status.
    $r = Invoke-SelfTest -Url 'http://127.0.0.1:1/health'
    Assert-That 'unreachable server fails' ($r.RESULT -eq 'FAIL')
    Assert-That 'unreachable server has no HTTP status' ($r.STATUS -eq '-') "status $($r.STATUS)"
    Assert-That 'unreachable server reports a WinHTTP code' ($r.CODE -ne '0') "code $($r.CODE)"
    # The point of resolving WinHTTP codes through winhttp.dll: without it this
    # message would be a bare hex number the user can do nothing with.
    Assert-That 'transport error is readable English, not just a number' `
    ($r.MESSAGE -match '[A-Za-z]{4,}\s+[A-Za-z]{2,}') "message was [$($r.MESSAGE)]"

    # ---- A credential must not be readable from the report -----------------
    Assert-That 'the access key never appears in the report' `
    (-not $r.MESSAGE.Contains('pcn_'))
} finally {
    if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -Recurse -Force $script:work -ErrorAction SilentlyContinue
}

Write-Host "`n=== $($script:checks - $script:failures.Count)/$($script:checks) checks passed ===" -ForegroundColor Cyan
if ($script:failures.Count -gt 0) {
    Write-Host 'FAILED:' -ForegroundColor Red
    $script:failures | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    exit 1
}
Write-Host 'HTTP client works against a live server.' -ForegroundColor Green
exit 0
