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

    Covers the outcomes the UI has to tell apart (installer/CLAUDE.md
    §Error handling, and task I8's access-key UX):
      200 with a body     — the happy path, Content-Length honoured
      404                 — package absent, OR a live key without the grant:
                            the server sends no 403, because that would say
                            "exists, but not for you" (DESIGN-REVIEW.md D3)
      401                 — no live key on a protected directory
      transport failure   — server unreachable, reported in words not a code

    And the two endpoints that are not downloads: GET /whoami, the only way
    to validate an access key, and POST /report/conflict, the only thing the
    installer ever sends.

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
    return ConvertFrom-Report -Path $out -ExitCode $proc.ExitCode
}

<#  Run one --selftest-report: the installer POSTs a fabricated conflict
    report through the same code an install uses. Same flat report shape. #>
function Invoke-SelfReport {
    param([Parameter(Mandatory)][string]$Base, [Parameter(Mandatory)][string]$Game)
    $out = Join-Path $script:work ("conflict-" + [guid]::NewGuid() + ".txt")
    $proc = Start-Process $exe -PassThru -Wait -ArgumentList @(
        '--selftest-report', $Game, '--server', $Base, '--out', $out)
    return ConvertFrom-Report -Path $out -ExitCode $proc.ExitCode
}

function ConvertFrom-Report {
    param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][int]$ExitCode)
    $report = @{ ExitCode = $ExitCode }
    foreach ($line in Get-Content $Path) {
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
# The server crate has its own lint policy, and changed RUSTFLAGS would rebuild its whole tree.
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
    # The same directory+name under two games, so every URL and key must carry the game.
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
    # A game without a schema publishes nothing; dirtrally2 has no canonical one, so it declares its category.
    # WriteAllText writes no BOM, which the server's TOML parser would reject.
    Copy-Item -LiteralPath (Join-Path $repoRoot 'schemas\rbr.toml') `
        -Destination (Join-Path $dataRoot 'rbr\schema.toml')
    [System.IO.File]::WriteAllText((Join-Path $dataRoot 'dirtrally2\schema.toml'),
        "[[category]]`nid = `"codriver`"`nselect = `"one`"`ninstall_root = `"game`"`n")

    & $server manifest gen --data $dataRoot | Out-Null
    $manifest = Get-Content -Raw (Join-Path $dataRoot 'manifest.json') | ConvertFrom-Json
    Assert-That 'server published all four packages' ($manifest.packages.Count -eq 4) `
        "manifest lists $($manifest.packages.Count)"
    $public = $manifest.packages | Where-Object { $_.directory -eq 'public' -and $_.game -eq 'rbr' }
    $protected = $manifest.packages | Where-Object { $_.directory -eq 'premium-voices' -and $_.game -eq 'rbr' }
    # The nested folders became the package's name, which is the UI tree.
    Assert-That 'a package name is its path under the access directory' `
    ($public.name -eq 'codriver/polish/male/Kamil') "name is $($public.name)"

    # --db-url belongs to `token`, not to `mint`.
    $db = "file:$(Join-Path $work 'auth.db')"
    $mint = (& $server token --db-url $db mint --label 'http-client-test' --grant 'premium-voices') -join "`n"
    $tokenMatch = [regex]::Match($mint, 'pcn_[A-Za-z0-9_\-]+')
    Assert-That 'server minted a credential' $tokenMatch.Success "mint said: $mint"
    if (-not $tokenMatch.Success) { throw 'cannot continue without a credential' }
    $token = $tokenMatch.Value

    # Its stdout is the access log, which is where the user agent ends up.
    $serverLog = Join-Path $work 'server.log'
    $proc = Start-Process $server -PassThru -WindowStyle Hidden -RedirectStandardOutput $serverLog -ArgumentList @(
        'serve', '--listen', "127.0.0.1:$Port", '--data', $dataRoot, '--db-url', $db)
    $base = "http://127.0.0.1:$Port"
    $deadline = (Get-Date).AddSeconds(15)
    do {
        Start-Sleep -Milliseconds 200
        $up = (Invoke-SelfTest -Url "$base/health").RESULT -eq 'OK'
    } while (-not $up -and (Get-Date) -lt $deadline)
    Assert-That 'server answers /health' $up

    Write-Host "`n-- GET /health --"
    $r = Invoke-SelfTest -Url "$base/health"
    Assert-That 'health succeeds' ($r.RESULT -eq 'OK') "report says $($r.RESULT)"
    Assert-That 'health exits 0' ($r.ExitCode -eq 0) "exit $($r.ExitCode)"
    Assert-That "health body is the server's 'ok'" ($r.BODY -eq 'ok') "got [$($r.BODY)]"
    Assert-That 'Content-Length matches the bytes read' ($r.CONTENT_LENGTH -eq $r.BYTES) `
        "declared $($r.CONTENT_LENGTH), read $($r.BYTES)"

    Write-Host "`n-- GET /packages (anonymous) --"
    $r = Invoke-SelfTest -Url "$base/packages?game=rbr"
    Assert-That 'packages succeeds' ($r.RESULT -eq 'OK')
    Assert-That 'anonymous listing names the public package' `
    ($r.BODY.Contains("`"name`":`"$($public.name)`"")) "body was [$($r.BODY)]"
    Assert-That 'anonymous listing hides the protected directory' `
    (-not $r.BODY.Contains('premium-voices')) "body was [$($r.BODY)]"
    # The installer verifies downloads against this sha.
    Assert-That 'listing carries the sha the server computed' `
    ($r.BODY.Contains($public.sha256))
    # The other game publishes the same directory+name.
    Assert-That '?game= filters other games out server-side' `
    (-not $r.BODY.Contains('dirtrally2')) "body was [$($r.BODY)]"

    Write-Host "`n-- GET /packages (with access key) --"
    $r = Invoke-SelfTest -Url "$base/packages?game=rbr" -Token $token
    Assert-That 'authenticated listing succeeds' ($r.RESULT -eq 'OK')
    Assert-That 'granted directory now appears' ($r.BODY.Contains('premium-voices')) `
        "body was [$($r.BODY)]"
    Assert-That 'bearer header reached the server (listing grew)' `
    ($r.BODY.Contains($protected.sha256))

    # ---- /whoami: the only way to validate a key ---------------------------
    # /packages answers a bad key with the public listing, so it cannot tell a typo from a good key.
    Write-Host "`n-- GET /packages (bad key) --"
    $r = Invoke-SelfTest -Url "$base/packages?game=rbr" -Token 'pcn_not_a_real_key'
    Assert-That 'a bad key still gets a 200 from /packages' ($r.RESULT -eq 'OK') `
        "status $(if ($r.ContainsKey('STATUS')) { $r.STATUS } else { '-' })"
    Assert-That '...and sees only the public directory' `
    (-not $r.BODY.Contains('premium-voices')) "body was [$($r.BODY)]"

    Write-Host "`n-- GET /whoami --"
    $r = Invoke-SelfTest -Url "$base/whoami"
    Assert-That 'whoami without a key fails' ($r.RESULT -eq 'FAIL')
    Assert-That 'whoami without a key is 401' ($r.STATUS -eq '401') "status $($r.STATUS)"

    $r = Invoke-SelfTest -Url "$base/whoami" -Token 'pcn_not_a_real_key'
    Assert-That 'whoami rejects an unknown key with 401' ($r.STATUS -eq '401') `
        "status $($r.STATUS), message $($r.MESSAGE)"

    $r = Invoke-SelfTest -Url "$base/whoami" -Token $token
    Assert-That 'whoami accepts the minted key' ($r.RESULT -eq 'OK') `
        "status $(if ($r.ContainsKey('STATUS')) { $r.STATUS } else { '-' })"
    # Present even when null, since scene 2 has to tell "never" from "soon".
    Assert-That 'whoami answers a KeyStatus' ($r.BODY.Contains('expires_at')) `
        "body was [$($r.BODY)]"
    Assert-That 'a key minted without a deadline never expires' `
    ($r.BODY -match '"expires_at"\s*:\s*null') "body was [$($r.BODY)]"

    # Minted after the server started, which also shows auth is read per request.
    $datedMint = (& $server token --db-url $db mint --label 'expiring' --expires-days 3) -join "`n"
    $datedMatch = [regex]::Match($datedMint, 'pcn_[A-Za-z0-9_\-]+')
    Assert-That 'server minted a dated credential' $datedMatch.Success "mint said: $datedMint"
    if ($datedMatch.Success) {
        $r = Invoke-SelfTest -Url "$base/whoami" -Token $datedMatch.Value
        Assert-That 'a dated key is live until then' ($r.RESULT -eq 'OK')
        Assert-That 'whoami reports the expiry as unix seconds' `
        ($r.BODY -match '"expires_at"\s*:\s*\d{10}') "body was [$($r.BODY)]"
    }

    # ---- POST /report/conflict --------------------------------------------
    # Fabricated, because a real report needs a catalogue where two packages write one file.
    Write-Host "`n-- POST /report/conflict --"
    $r = Invoke-SelfReport -Base $base -Game 'rbr'
    Assert-That 'the conflict report is accepted' ($r.RESULT -eq 'OK') `
        "status $(if ($r.ContainsKey('STATUS')) { $r.STATUS } else { '-' }), message $(if ($r.ContainsKey('MESSAGE')) { $r.MESSAGE } else { '' })"
    Assert-That 'reporting exits 0' ($r.ExitCode -eq 0) "exit $($r.ExitCode)"
    $stored = Get-ChildItem -Path (Join-Path $dataRoot 'reports') -Filter '*.jsonl' `
        -ErrorAction SilentlyContinue | ForEach-Object { Get-Content -Raw $_.FullName }
    Assert-That 'the server stored the report' ($null -ne $stored) 'no reports/*.jsonl'
    if ($stored) {
        $line = ($stored -join '')
        Assert-That 'it carries the game and the clashing file' `
        ($line.Contains('"rbr"') -and $line.Contains('PaceNote.ini')) "stored: $line"
        # The report names its game once, so writers are directory/name refs.
        Assert-That 'writers are directory/name refs in install order' `
        ($line -match '"public/language/selftest"\s*,\s*"public/codriver/selftest"') "stored: $line"
        Assert-That 'the report carries no access key' (-not $line.Contains('pcn_')) "stored: $line"
        Assert-That 'the server stamped its own time' ($line -match '"(received_at|at)"\s*:') `
            "stored: $line"
    }

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

    Write-Host "`n-- error surfaces --"
    $r = Invoke-SelfTest -Url "$base/download/rbr/public/no-such-package"
    Assert-That 'missing package fails' ($r.RESULT -eq 'FAIL')
    Assert-That 'missing package reports 404' ($r.STATUS -eq '404') "status $($r.STATUS)"
    Assert-That 'missing package exits non-zero' ($r.ExitCode -ne 0)

    # A real directory+name under a game that lacks it must not serve the other game's bytes.
    $r = Invoke-SelfTest -Url "$base/download/beamng/$($public.directory)/$($public.name)"
    Assert-That 'the same directory+name under another game is 404' `
    ($r.STATUS -eq '404') "status $($r.STATUS), message $($r.MESSAGE)"

    # Port 1 has nothing listening: a transport failure, with no HTTP status.
    $r = Invoke-SelfTest -Url 'http://127.0.0.1:1/health'
    Assert-That 'unreachable server fails' ($r.RESULT -eq 'FAIL')
    Assert-That 'unreachable server has no HTTP status' ($r.STATUS -eq '-') "status $($r.STATUS)"
    Assert-That 'unreachable server reports a WinHTTP code' ($r.CODE -ne '0') "code $($r.CODE)"
    # WinHTTP codes resolve through winhttp.dll, or this would be a bare number.
    Assert-That 'transport error is readable English, not just a number' `
    ($r.MESSAGE -match '[A-Za-z]{4,}\s+[A-Za-z]{2,}') "message was [$($r.MESSAGE)]"

    Assert-That 'the access key never appears in the report' `
    (-not $r.MESSAGE.Contains('pcn_'))

    Write-Host "`n-- user agent --"
    # The binary's own product version, which Test-ReleaseVersion ties to the release tag.
    $version = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($exe).ProductVersion
    $agent = "Pacenotes-Installer/$version"
    $logged = if (Test-Path $serverLog) { Get-Content -Raw $serverLog } else { '' }
    Assert-That "the server's access log names this build ($agent)" `
    ($logged -match "agent=$([regex]::Escape($agent))(\s|$)") 'no access line carried that agent'
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
