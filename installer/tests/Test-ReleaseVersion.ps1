<#
.SYNOPSIS
    AUTOMATED TEST. A release's version comes from its tag, `v/X.Y.Z`, and
    only from there. Exits non-zero if a build disagrees with the tag it was
    built from.

.DESCRIPTION
    The tag is the one place a version is written: the CD workflow passes it
    to the build as PACENOTES_RELEASE_TAG, and build.rs turns it into the user
    agent, the conflict report's version and all four VERSIONINFO fields.

    So this test builds rather than reads files. It builds from a real tag and
    from none, and reads the version back out of what was built: the file
    properties Explorer shows, and the user agent string in the binary. Then
    it checks that every malformed tag fails the build, since a release built
    from a mistyped tag must not exist.

    Debug builds, which carry the same resources and constants as a release
    and take seconds once the dependencies are compiled.

.EXAMPLE
    pwsh -File tests/Test-ReleaseVersion.ps1
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$crateRoot = Split-Path -Parent $PSScriptRoot
$exePath = Join-Path $crateRoot 'target\x86_64-pc-windows-msvc\debug\pacenotes-installer.exe'
$TAG_KEY = 'PACENOTES_RELEASE_TAG'

$failures = New-Object System.Collections.ArrayList
$checks = 0
function Assert-That {
    param([string]$Name, [bool]$Condition, [string]$Detail = '')
    $script:checks++
    if ($Condition) {
        Write-Host "  PASS  $Name" -ForegroundColor Green
    } else {
        Write-Host ("  FAIL  {0}{1}" -f $Name, $(if ($Detail) { " -- $Detail" } else { '' })) -ForegroundColor Red
        [void]$failures.Add($Name)
    }
}

<#  Build with the tag set (or unset for $null); returns the exit code and output. #>
function Invoke-Build {
    # Untyped: a [string] parameter turns $null into "", which is a different case.
    param($Tag)
    if ($null -eq $Tag) {
        Remove-Item "env:$TAG_KEY" -ErrorAction SilentlyContinue
    } else {
        Set-Item "env:$TAG_KEY" $Tag
    }
    Push-Location $crateRoot
    try {
        $output = & cargo build --locked --target x86_64-pc-windows-msvc 2>&1 | Out-String
        return [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = $output }
    } finally {
        Pop-Location
    }
}

<#  The user agent as the binary holds it, or $null when that string is absent. #>
function Test-AgentInBinary {
    param([string]$Agent)
    $bytes = [System.IO.File]::ReadAllBytes($exePath)
    $text = [System.Text.Encoding]::Latin1.GetString($bytes)
    return $text.Contains($Agent)
}

<#  Build from $Tag and assert the binary says $Product / $Numbers everywhere. #>
function Assert-BuiltVersion {
    param($Tag, [string]$Product, [int[]]$Numbers)
    $label = if ($null -eq $Tag) { 'no tag' } else { $Tag }
    $build = Invoke-Build $Tag
    Assert-That "a build from $label succeeds" ($build.ExitCode -eq 0) $build.Output
    if ($build.ExitCode -ne 0) { return }

    $info = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($exePath)
    $fileVersion = '{0}.{1}.{2}.0' -f $Numbers[0], $Numbers[1], $Numbers[2]
    Assert-That "its product version is $Product" ($info.ProductVersion -eq $Product) "got [$($info.ProductVersion)]"
    Assert-That "its file version is $fileVersion" ($info.FileVersion -eq $fileVersion) "got [$($info.FileVersion)]"
    $parts = @($info.FileMajorPart, $info.FileMinorPart, $info.FileBuildPart, $info.FilePrivatePart)
    $expected = @($Numbers[0], $Numbers[1], $Numbers[2], 0)
    Assert-That 'and its numeric file version agrees' (-not (Compare-Object $parts $expected)) `
        "got [$($parts -join ',')]"
    $agent = "Pacenotes-Installer/$Product"
    Assert-That "its user agent is $agent" (Test-AgentInBinary $agent)
}

Write-Host '=== release version (tag -> build) ===' -ForegroundColor Cyan
$previous = [Environment]::GetEnvironmentVariable($TAG_KEY)
try {
    Write-Host "`n-- a release tag --"
    Assert-BuiltVersion -Tag 'v/7.8.9' -Product '7.8.9' -Numbers @(7, 8, 9)
    # The largest a VERSIONINFO word holds.
    Assert-BuiltVersion -Tag 'v/65535.0.12' -Product '65535.0.12' -Numbers @(65535, 0, 12)

    Write-Host "`n-- no tag: a development build --"
    Assert-BuiltVersion -Tag $null -Product '0.0.0-dev' -Numbers @(0, 0, 0)

    Write-Host "`n-- a malformed tag fails the build --"
    $malformed = @(
        'v7.8.9',        # the old shape, without the slash
        '7.8.9',
        'v/7.8',
        'v/7.8.9.1',
        'v/7.8.x',
        'v/7.8.9-rc1',
        'v/+7.8.9',
        'v/65536.0.0',  # does not fit a VERSIONINFO word
        ''              # set but empty: a release job that lost its tag
    )
    foreach ($tag in $malformed) {
        $build = Invoke-Build $tag
        Assert-That "[$tag] is refused" ($build.ExitCode -ne 0)
        Assert-That "  and the error names $TAG_KEY" ($build.Output.Contains($TAG_KEY))
    }
} finally {
    if ($null -eq $previous) {
        Remove-Item "env:$TAG_KEY" -ErrorAction SilentlyContinue
    } else {
        Set-Item "env:$TAG_KEY" $previous
    }
}

Write-Host "`n=== $($checks - $failures.Count)/$checks checks passed ===" -ForegroundColor Cyan
if ($failures.Count -gt 0) {
    Write-Host 'FAILED:' -ForegroundColor Red
    $failures | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    exit 1
}
Write-Host 'Every build carries the version of the tag it was built from.' -ForegroundColor Green
exit 0
