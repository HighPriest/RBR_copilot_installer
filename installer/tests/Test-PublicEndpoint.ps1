<#
.SYNOPSIS
    MANUAL CHECK. Probes the REAL public server over HTTPS and reports what
    each endpoint does. Exits non-zero if something is wrong.

.DESCRIPTION
    A third kind of script, distinct from the two in tests/README.md:

      AUTOMATED TEST  asserts, hermetic, safe to gate CI on
      AGENT AID       produces artifacts, asserts nothing
      MANUAL CHECK    asserts, but against something OUTSIDE this repo

    Never part of Run-AllTests.ps1 or CI: it depends on a machine someone else
    runs. Run it by hand when the deployment changes; it is the only test that
    takes WinHTTP's TLS path (see tests/README.md).

.PARAMETER Server
    Base URL. Defaults to PACENOTES_DEFAULT_SERVER in installer/.env, the
    address the build compiles in.

.PARAMETER Token
    Optional access key, passed to the installer as --token. Never stored.

.EXAMPLE
    pwsh -File tests/Test-PublicEndpoint.ps1
    pwsh -File tests/Test-PublicEndpoint.ps1 -Server https://staging.example.com
#>
[CmdletBinding()]
param(
    [string]$Server,
    [string]$Token,
    [string]$ExePath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$crateRoot = Split-Path -Parent $PSScriptRoot
if (-not $Server) {
    $envFile = Join-Path $crateRoot '.env'
    $setting = if (Test-Path $envFile) {
        Select-String -Path $envFile -Pattern '^\s*PACENOTES_DEFAULT_SERVER\s*=\s*"?([^"]+?)"?\s*$'
    }
    if (-not $setting) {
        Write-Error "Pass -Server, or set PACENOTES_DEFAULT_SERVER in installer/.env (see DEVELOPMENT.md)."
        exit 1
    }
    $Server = $setting.Matches[0].Groups[1].Value
}
if (-not $ExePath) {
    $ExePath = Join-Path $crateRoot 'target\x86_64-pc-windows-msvc\release\pacenotes-installer.exe'
}
if (-not (Test-Path $ExePath)) {
    Write-Error "Installer not built. Run: cargo build --release --target x86_64-pc-windows-msvc"
    exit 1
}
$Server = $Server.TrimEnd('/')
$work = Join-Path ([System.IO.Path]::GetTempPath()) ("pacenotes-public-" + [guid]::NewGuid())
[void](New-Item -ItemType Directory -Path $work)

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

# One --selftest-http run, parsed: the wizard's own WinHTTP code, not a PowerShell request.
function Invoke-SelfTest {
    param([Parameter(Mandatory)][string]$Url)
    $out = Join-Path $work ("r-" + [guid]::NewGuid() + ".txt")
    $cliArgs = @('--selftest-http', $Url, '--out', $out)
    if ($Token) { $cliArgs += @('--token', $Token) }
    $proc = Start-Process $ExePath -ArgumentList $cliArgs -PassThru -Wait
    $report = @{ ExitCode = $proc.ExitCode }
    foreach ($line in Get-Content $out) {
        $m = [regex]::Match($line, '^(?<k>[A-Z_]+): ?(?<v>.*)$')
        if ($m.Success) { $report[$m.Groups['k'].Value] = $m.Groups['v'].Value }
    }
    return $report
}

<#  A report only carries the keys its outcome produced — a success has no
    CODE, a transport failure has no BODY — and Set-StrictMode turns a missing
    key into a thrown error. #>
function Get-ReportField {
    param([Parameter(Mandatory)]$Report, [Parameter(Mandatory)][string]$Key, [string]$Default = '')
    if ($Report.ContainsKey($Key)) { return [string]$Report[$Key] }
    return $Default
}

Write-Host "=== Public endpoint: $Server ===" -ForegroundColor Cyan
Write-Host "exe: $ExePath`n"

try {
    # Every later check depends on the handshake.
    Write-Host '-- TLS handshake --'
    $r = Invoke-SelfTest -Url "$Server/health"
    $tlsOk = (Get-ReportField $r CODE) -ne '12175'
    Assert-That 'the TLS handshake completes (WinHTTP -> Schannel)' $tlsOk `
    ("WinHTTP {0}: {1}" -f (Get-ReportField $r CODE '-'), (Get-ReportField $r MESSAGE))
    if (-not $tlsOk) {
        Write-Host @'

  WinHTTP 12175 is ERROR_WINHTTP_SECURE_FAILURE: the handshake never
  completed, so nothing below can run. This is almost always the SERVER, not
  the client — most often the reverse proxy has no certificate for the
  hostname being requested, which makes it answer with a TLS internal-error
  alert. Check that the Caddyfile has a site block whose address matches this
  exact hostname, and that its certificate has been issued.

'@ -ForegroundColor Yellow
        throw 'TLS failed; skipping the rest'
    }

    Assert-That '/health answers 200' ((Get-ReportField $r RESULT) -eq 'OK') "report says $(Get-ReportField $r RESULT)"
    Assert-That "/health says 'ok'" ((Get-ReportField $r BODY) -eq 'ok') "got [$(Get-ReportField $r BODY)]"

    Write-Host "`n-- catalogue --"
    $r = Invoke-SelfTest -Url "$Server/packages?game=rbr"
    Assert-That '/packages?game=rbr answers' ((Get-ReportField $r RESULT) -eq 'OK') "report says $(Get-ReportField $r MESSAGE)"
    if ((Get-ReportField $r RESULT) -eq 'OK') {
        # BODY is truncated to a preview, so report rather than parse it.
        $body = Get-ReportField $r BODY
        Assert-That 'the catalogue lists packages' ($body -like '*"packages"*') "body began [$body]"
        # .Contains, not -like: "[]" is a wildcard character class (tests/README.md).
        Assert-That 'the catalogue is not empty' (-not $body.Contains('"packages":[]')) "body began [$body]"
        Write-Host "    bytes: $(Get-ReportField $r BYTES)"
        Write-Host "    first of it: $($body.Substring(0, [Math]::Min(220, $body.Length)))"
    }

    # A 404 is a valid answer here, not a failure.
    Write-Host "`n-- message of the day --"
    $r = Invoke-SelfTest -Url "$Server/motd?lang=en"
    if ((Get-ReportField $r RESULT) -eq 'OK') {
        Assert-That '/motd returns text' ([int](Get-ReportField $r BYTES 0) -gt 0)
        Write-Host "    says: $(Get-ReportField $r BODY)"
    } else {
        Assert-That '/motd is 404 (none published) rather than broken' ((Get-ReportField $r STATUS) -eq '404') `
            "status $(Get-ReportField $r STATUS), message $(Get-ReportField $r MESSAGE)"
        Write-Host '    none published; scene 2 shows its built-in offline copy.'
    }
} catch {
    # Register it: a run that stopped early must never reach the success line.
    Assert-That "the check ran to completion" $false $_.Exception.Message
} finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}

Write-Host "`n=== $($script:checks - $script:failures.Count)/$($script:checks) checks passed ===" -ForegroundColor Cyan
if ($script:failures.Count -gt 0) {
    Write-Host 'FAILED:' -ForegroundColor Red
    $script:failures | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    exit 1
}
Write-Host 'The public endpoint is reachable over HTTPS.' -ForegroundColor Green
exit 0
