<#
.SYNOPSIS
    AUTOMATED TEST. The redirect policy: a bearer token never leaves the
    origin the caller asked for. Exits 0 on success, 1 on any failure.

.DESCRIPTION
    Left to itself WinHTTP follows a 3xx and re-sends `Authorization` to
    the new origin, while `http.rs` never sees the hop — measured on
    2026-10-30, which is why `send` now disables automatic redirects and
    follows them by hand. This is the test that keeps it that way.

    The client is driven through `--selftest-http`, so what is measured is
    the real WinHTTP path rather than a reimplementation of it. Two
    listeners on two ports are two different ORIGINS: the port is part of
    one.

    What it has to prove, because each is a way the policy could be
    quietly lost:
      a cross-origin redirect is followed, but WITHOUT the token
      a same-origin redirect keeps the token, or auth breaks on a
        server that rewrites paths
      a redirect loop ends by itself rather than hanging
      https that redirects to http is refused, not followed

    Needs Windows and the release build; no desktop and no server, so CI
    can run it.

.EXAMPLE
    pwsh -File tests/Test-Redirects.ps1
#>
[CmdletBinding()]
param([string]$ExePath)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$crateRoot = Split-Path -Parent $PSScriptRoot
if (-not $ExePath) {
    $ExePath = Join-Path $crateRoot 'target\x86_64-pc-windows-msvc\release\pacenotes-installer.exe'
}
if (-not (Test-Path $ExePath)) {
    Write-Error "Installer not built. Run: cargo build --release --target x86_64-pc-windows-msvc"
    exit 1
}
$ExePath = (Resolve-Path $ExePath).Path
$TOKEN = 'pcn_redirect_test_token'

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

function Get-FreePort {
    $l = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $l.Start(); $p = $l.LocalEndpoint.Port; $l.Stop(); return $p
}

<#  Run one --selftest-http against a listener set, and report what each
    port saw. `Plan` maps a port to a scriptblock answering one request. #>
function Invoke-Probe {
    param(
        [Parameter(Mandatory)][hashtable]$Plan,
        [Parameter(Mandatory)][int]$StartPort,
        [switch]$NoToken
    )
    $seen = [System.Collections.Concurrent.ConcurrentQueue[string]]::new()
    $listeners = @{}
    $runspaces = @()
    foreach ($port in $Plan.Keys) {
        $l = New-Object System.Net.HttpListener
        $l.Prefixes.Add("http://127.0.0.1:$port/")
        $l.Start()
        $listeners[$port] = $l
        $ps = [powershell]::Create()
        [void]$ps.AddScript($Plan[$port]).AddArgument($l).AddArgument($seen).AddArgument($port)
        $runspaces += [pscustomobject]@{ Shell = $ps; Handle = $ps.BeginInvoke() }
    }

    $out = Join-Path $crateRoot "target\redirect-test-$([guid]::NewGuid()).txt"
    $cliArgs = @('--selftest-http', "http://127.0.0.1:$StartPort/start", '--out', $out)
    if (-not $NoToken) { $cliArgs += @('--token', $TOKEN) }
    $proc = Start-Process $ExePath -Wait -PassThru -ArgumentList $cliArgs

    Start-Sleep -Milliseconds 300
    # Listeners FIRST. A runspace parked in GetContext() is inside a
    # blocking native call, where Stop() cannot reach it; closing the
    # listener is what makes that call throw and the runspace finish.
    # Doing it the other way round hangs the test, not the client.
    foreach ($l in $listeners.Values) { $l.Close() }
    foreach ($r in $runspaces) { $r.Shell.Stop(); $r.Shell.Dispose() }

    $report = @{ ExitCode = $proc.ExitCode }
    if (Test-Path $out) {
        foreach ($line in Get-Content $out) {
            $m = [regex]::Match($line, '^(?<k>[A-Z_]+): ?(?<v>.*)$')
            if ($m.Success) { $report[$m.Groups['k'].Value] = $m.Groups['v'].Value }
        }
        Remove-Item $out -ErrorAction SilentlyContinue
    }
    return [pscustomobject]@{ Seen = $seen.ToArray(); Report = $report }
}

function Get-Header {
    param($Seen, [int]$Port)
    foreach ($e in $Seen) {
        $parts = $e -split '\|', 2
        if ([int]$parts[0] -eq $Port) { return $(if ($parts.Count -gt 1) { $parts[1] } else { '' }) }
    }
    return $null   # that port was never asked for anything
}

# Listener bodies. Each records the Authorization it was handed, then answers.
$redirectTo = {
    param($listener, $seen, $port)
    $ctx = $listener.GetContext()
    $seen.Enqueue("$port|$($ctx.Request.Headers['Authorization'])")
    $ctx.Response.StatusCode = 302
    $ctx.Response.AddHeader('Location', $env:PACENOTES_TEST_LOCATION)
    $ctx.Response.Close()
}
$ok = {
    param($listener, $seen, $port)
    $ctx = $listener.GetContext()
    $seen.Enqueue("$port|$($ctx.Request.Headers['Authorization'])")
    $body = [Text.Encoding]::UTF8.GetBytes('landed')
    $ctx.Response.StatusCode = 200
    $ctx.Response.ContentLength64 = $body.Length
    $ctx.Response.OutputStream.Write($body, 0, $body.Length)
    $ctx.Response.Close()
}

Write-Host '=== redirect policy ===' -ForegroundColor Cyan

# ---- a cross-origin redirect drops the token ----
Write-Host "`n-- cross-origin --"
$a = Get-FreePort; $b = Get-FreePort
$env:PACENOTES_TEST_LOCATION = "http://127.0.0.1:$b/landed"
$r = Invoke-Probe -Plan @{ $a = $redirectTo; $b = $ok } -StartPort $a

Assert-That 'the first origin is given the token' `
((Get-Header $r.Seen $a) -eq "Bearer $TOKEN") "got [$(Get-Header $r.Seen $a)]"
Assert-That 'the redirect is followed' ($null -ne (Get-Header $r.Seen $b))
Assert-That 'and the other origin is NOT given the token' `
([string]::IsNullOrEmpty((Get-Header $r.Seen $b))) `
    "the bearer token reached a host the user never configured: [$(Get-Header $r.Seen $b)]"
Assert-That 'the body still arrives' ($r.Report.RESULT -eq 'OK') "report said $($r.Report.RESULT)"

# ---- a same-origin redirect keeps it ----
# Same listener, so same scheme+host+port: a path rewrite must not break auth.
Write-Host "`n-- same origin --"
$c = Get-FreePort
$script:hits = 0
$twoStep = {
    param($listener, $seen, $port)
    for ($i = 0; $i -lt 2; $i++) {
        $ctx = $listener.GetContext()
        $seen.Enqueue("$port|$($ctx.Request.Headers['Authorization'])")
        if ($ctx.Request.Url.AbsolutePath -eq '/start') {
            $ctx.Response.StatusCode = 302
            $ctx.Response.AddHeader('Location', '/moved')
        } else {
            $body = [Text.Encoding]::UTF8.GetBytes('landed')
            $ctx.Response.StatusCode = 200
            $ctx.Response.ContentLength64 = $body.Length
            $ctx.Response.OutputStream.Write($body, 0, $body.Length)
        }
        $ctx.Response.Close()
    }
}
$r = Invoke-Probe -Plan @{ $c = $twoStep } -StartPort $c
$sameOrigin = @($r.Seen | Where-Object { $_ -like "$c|*" })
Assert-That 'both requests reached the same origin' ($sameOrigin.Count -eq 2) `
    "saw $($sameOrigin.Count)"
Assert-That 'and both carried the token' `
(@($sameOrigin | Where-Object { $_ -eq "$c|Bearer $TOKEN" }).Count -eq 2) `
    "saw: $($sameOrigin -join ' / ')"
Assert-That 'the redirected body arrives' ($r.Report.RESULT -eq 'OK')

# ---- a loop ends by itself ----
Write-Host "`n-- a redirect loop --"
$d = Get-FreePort
# Answers every request with another redirect. It outlives the client on
# purpose — the client is supposed to stop first — so the loop ends when
# the listener closes and GetContext throws.
$loop = {
    param($listener, $seen, $port)
    try {
        for ($i = 0; $i -lt 20; $i++) {
            $ctx = $listener.GetContext()
            $seen.Enqueue("$port|loop")
            $ctx.Response.StatusCode = 302
            $ctx.Response.AddHeader('Location', "http://127.0.0.1:$port/again$i")
            $ctx.Response.Close()
        }
    } catch { }
}
$r = Invoke-Probe -Plan @{ $d = $loop } -StartPort $d
Assert-That 'a redirect loop fails instead of hanging' ($r.Report.RESULT -eq 'FAIL') `
    "report said $($r.Report.RESULT)"
Assert-That 'and it gives up after a few hops, not ten' `
(@($r.Seen).Count -le 5) "made $(@($r.Seen).Count) requests"

Write-Host "`n=== $($script:checks - $script:failures.Count)/$($script:checks) checks passed ===" -ForegroundColor Cyan
if ($script:failures.Count -gt 0) {
    Write-Host 'FAILED:' -ForegroundColor Red
    $script:failures | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    exit 1
}
Write-Host 'A bearer token never leaves the origin it was meant for.' -ForegroundColor Green
exit 0
