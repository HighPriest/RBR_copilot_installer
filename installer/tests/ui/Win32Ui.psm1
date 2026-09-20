<#
.SYNOPSIS
    Shared Win32 UI-driving helpers for the installer's GUI scripts.

.DESCRIPTION
    SUPPORT MODULE. Not a test by itself — imported by both the automated test
    (Test-UiLayout.ps1) and the agent aid (Get-UiScreenshot.ps1).

    The installer is a raw Win32 dialog app with no automation interface, so
    these helpers poke it through user32.dll: enumerate windows and controls,
    read and write control text, and drive tabs.

    Two gotchas encoded here, both learned the hard way:

    1. Output written from inside a P/Invoke delegate never reaches the
       PowerShell pipeline. Enumeration therefore fills a script-scoped
       accumulator and returns it afterwards.
    2. PowerShell variable names are CASE-INSENSITIVE. A local named $tab will
       silently clobber a parameter named $Tab. Helper parameters here are
       named distinctly for that reason.
#>

Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;

public struct RECT { public int L, T, R, B; }

public static class NativeUi {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr h, EnumProc f, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int m);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int m);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
  [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")]
  public static extern IntPtr SendMessageString(IntPtr h, uint m, IntPtr w, string l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")]
  public static extern IntPtr SendMessageGetText(IntPtr h, uint m, IntPtr w, StringBuilder l);
  [DllImport("user32.dll")] public static extern IntPtr PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr SetFocus(IntPtr h);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
}
"@

# ---- Window messages / notifications used by the scripts ----
$script:WM_SETTEXT = 0x000C
$script:WM_CLOSE = 0x0010
$script:WM_COMMAND = 0x0111
$script:WM_KEYDOWN = 0x0100
$script:WM_KEYUP = 0x0101
$script:CB_GETCOUNT = 0x0146
$script:CB_GETCURSEL = 0x0147
$script:CB_SETCURSEL = 0x014E
$script:LB_ADDSTRING = 0x0180
$script:LB_SETCURSEL = 0x0186
$script:TCM_GETITEMCOUNT = 0x1304
$script:TCM_GETCURSEL = 0x130B
$script:VK_RIGHT = 0x27
$script:VK_LEFT = 0x25

function Get-ProcessDialogs {
    <#  All visible top-level dialogs ("#32770") owned by a process. #>
    param([Parameter(Mandatory)][int]$OwnerPid)
    $script:acc = New-Object System.Collections.ArrayList
    $cb = [NativeUi+EnumProc] {
        param($h, $l)
        $q = 0
        [void][NativeUi]::GetWindowThreadProcessId($h, [ref]$q)
        if ($q -eq $script:ownerPidFilter -and [NativeUi]::IsWindowVisible($h)) {
            $c = New-Object System.Text.StringBuilder 64
            [void][NativeUi]::GetClassNameW($h, $c, 64)
            if ($c.ToString() -eq '#32770') {
                $t = New-Object System.Text.StringBuilder 512
                [void][NativeUi]::GetWindowTextW($h, $t, 512)
                [void]$script:acc.Add([pscustomobject]@{ Handle = $h; Title = $t.ToString() })
            }
        }
        return $true
    }
    $script:ownerPidFilter = $OwnerPid
    [void][NativeUi]::EnumWindows($cb, [IntPtr]::Zero)
    return $script:acc
}

function Wait-ForDialog {
    <#  Poll until a dialog shows up, instead of a blind sleep. #>
    param(
        [Parameter(Mandatory)][int]$OwnerPid,
        [string]$NotHandle = '',
        [int]$TimeoutMs = 5000
    )
    $deadline = (Get-Date).AddMilliseconds($TimeoutMs)
    while ((Get-Date) -lt $deadline) {
        $found = @(Get-ProcessDialogs -OwnerPid $OwnerPid |
            Where-Object { $NotHandle -eq '' -or "$($_.Handle)" -ne $NotHandle })
        if ($found.Count -gt 0) { return $found[0] }
        Start-Sleep -Milliseconds 100
    }
    return $null
}

function Get-ControlInventory {
    <#  Every child control: id, class, visibility, text. #>
    param([Parameter(Mandatory)][IntPtr]$Dialog)
    $script:inv = New-Object System.Collections.ArrayList
    $cb = [NativeUi+EnumProc] {
        param($h, $l)
        $c = New-Object System.Text.StringBuilder 64
        [void][NativeUi]::GetClassNameW($h, $c, 64)
        $t = New-Object System.Text.StringBuilder 1024
        [void][NativeUi]::GetWindowTextW($h, $t, 1024)
        [void]$script:inv.Add([pscustomobject]@{
                Id      = [NativeUi]::GetDlgCtrlID($h)
                Class   = $c.ToString()
                Visible = [NativeUi]::IsWindowVisible($h)
                Text    = $t.ToString()
                Handle  = $h
            })
        return $true
    }
    [void][NativeUi]::EnumChildWindows($Dialog, $cb, [IntPtr]::Zero)
    return $script:inv
}

function Get-CtrlText {
    <#  Reads control text with WM_GETTEXT, NOT GetWindowText.

        GetWindowText deliberately does not cross process boundaries: for a
        window owned by another process it returns the cached window title
        only. Static and Button captions live in that cache so they read fine,
        but Edit and ListBox contents do not, and come back as empty strings —
        which silently turns assertions into false passes. WM_GETTEXT is
        marshalled between processes by USER32, so it returns the real text.
    #>
    param([Parameter(Mandatory)][IntPtr]$Dialog, [Parameter(Mandatory)][int]$CtrlId)
    $h = [NativeUi]::GetDlgItem($Dialog, $CtrlId)
    if ($h -eq [IntPtr]::Zero) { return $null }
    $cap = 4096
    $sb = New-Object System.Text.StringBuilder $cap
    [void][NativeUi]::SendMessageGetText($h, 0x000D, [IntPtr]$cap, $sb)   # WM_GETTEXT
    return $sb.ToString()
}

function Set-CtrlText {
    <#  WM_SETTEXT is one of the messages USER32 marshals across processes. #>
    param([Parameter(Mandatory)][IntPtr]$Dialog, [Parameter(Mandatory)][int]$CtrlId, [string]$Value = '')
    $h = [NativeUi]::GetDlgItem($Dialog, $CtrlId)
    [void][NativeUi]::SendMessageString($h, $script:WM_SETTEXT, [IntPtr]::Zero, $Value)
}

function Test-CtrlVisible {
    param([Parameter(Mandatory)][IntPtr]$Dialog, [Parameter(Mandatory)][int]$CtrlId)
    return [NativeUi]::IsWindowVisible([NativeUi]::GetDlgItem($Dialog, $CtrlId))
}

function Invoke-Button {
    <#  Synthesize BN_CLICKED (HIWORD 0) for a control id. #>
    param([Parameter(Mandatory)][IntPtr]$Dialog, [Parameter(Mandatory)][int]$CtrlId)
    [void][NativeUi]::SendMessageW($Dialog, $script:WM_COMMAND, [IntPtr]$CtrlId, [IntPtr]::Zero)
}

function Invoke-EditChanged {
    <#  Synthesize EN_CHANGE (HIWORD 0x300) after a scripted Set-CtrlText,
        which does not itself raise the notification cross-process. #>
    param([Parameter(Mandatory)][IntPtr]$Dialog, [Parameter(Mandatory)][int]$CtrlId)
    $wparam = [IntPtr]((0x0300 -shl 16) -bor $CtrlId)
    [void][NativeUi]::SendMessageW($Dialog, $script:WM_COMMAND, $wparam, [IntPtr]::Zero)
}

function Get-TabSelection {
    param([Parameter(Mandatory)][IntPtr]$Dialog, [Parameter(Mandatory)][int]$TabCtrlId)
    $h = [NativeUi]::GetDlgItem($Dialog, $TabCtrlId)
    return [int][NativeUi]::SendMessageW($h, $script:TCM_GETCURSEL, [IntPtr]::Zero, [IntPtr]::Zero)
}

function Get-TabCount {
    param([Parameter(Mandatory)][IntPtr]$Dialog, [Parameter(Mandatory)][int]$TabCtrlId)
    $h = [NativeUi]::GetDlgItem($Dialog, $TabCtrlId)
    return [int][NativeUi]::SendMessageW($h, $script:TCM_GETITEMCOUNT, [IntPtr]::Zero, [IntPtr]::Zero)
}

function Select-Tab {
    <#  Move a tab selection with ARROW KEYS rather than TCM_SETCURSEL.

        TCM_SETCURSEL changes the selection without notifying the parent, so
        the app's TCN_SELCHANGE handler would never run and the test would
        prove nothing. A synthetic WM_NOTIFY is not an option either: it needs
        an NMHDR in the target process's address space. Arrow keys make the
        tab control itself raise TCN_SELCHANGE, which is what we want to test.
    #>
    param(
        [Parameter(Mandatory)][IntPtr]$Dialog,
        [Parameter(Mandatory)][int]$TabCtrlId,
        [Parameter(Mandatory)][int]$Index
    )
    $h = [NativeUi]::GetDlgItem($Dialog, $TabCtrlId)
    [void][NativeUi]::SetForegroundWindow($Dialog)
    Start-Sleep -Milliseconds 120
    [void][NativeUi]::SendMessageW($Dialog, 0x0028, $h, [IntPtr]1)   # WM_NEXTDLGCTL -> focus
    [void][NativeUi]::SetFocus($h)
    Start-Sleep -Milliseconds 120
    $guard = 0
    while ((Get-TabSelection -Dialog $Dialog -TabCtrlId $TabCtrlId) -ne $Index -and $guard -lt 12) {
        $cur = Get-TabSelection -Dialog $Dialog -TabCtrlId $TabCtrlId
        $key = if ($Index -gt $cur) { $script:VK_RIGHT } else { $script:VK_LEFT }
        [void][NativeUi]::PostMessageW($h, $script:WM_KEYDOWN, [IntPtr]$key, [IntPtr]::Zero)
        [void][NativeUi]::PostMessageW($h, $script:WM_KEYUP, [IntPtr]$key, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 220
        $guard++
    }
    return (Get-TabSelection -Dialog $Dialog -TabCtrlId $TabCtrlId)
}

function Add-ListItem {
    <#  Push an item into a listbox so selection-dependent paths can be tested
        before the real package lists exist (they arrive in task I5). #>
    param([Parameter(Mandatory)][IntPtr]$Dialog, [Parameter(Mandatory)][int]$CtrlId, [string]$Value = 'test item')
    $h = [NativeUi]::GetDlgItem($Dialog, $CtrlId)
    [void][NativeUi]::SendMessageString($h, $script:LB_ADDSTRING, [IntPtr]::Zero, $Value)
    [void][NativeUi]::SendMessageW($h, $script:LB_SETCURSEL, [IntPtr]0, [IntPtr]::Zero)
}

function Close-Dialog {
    param([Parameter(Mandatory)][IntPtr]$Dialog)
    [void][NativeUi]::PostMessageW($Dialog, $script:WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero)
}

function Get-ResourceStrings {
    <#
    .SYNOPSIS
        Read the expected UI text for one language straight out of
        res/installer.rc.

    .DESCRIPTION
        The resource script is the single source of truth for what the UI
        should say. Copying its strings into the test would only assert that
        two hand-written copies match, and would keep passing if a caption were
        wired to the wrong id or a translation were never added. Reading the
        .rc instead makes the assertion "the control shows what the resource
        defines for this language", which is the property worth holding.

        Returns a hashtable of IDS_NAME -> text, with \n and \r\n unescaped.
    #>
    param(
        [Parameter(Mandatory)][ValidateSet('en', 'pl')][string]$Language,
        [string]$RcPath
    )
    if (-not $RcPath) { $RcPath = Join-Path $PSScriptRoot '..\..\res\installer.rc' }
    $text = Get-Content -Raw -Encoding UTF8 (Resolve-Path $RcPath).Path

    $langToken = if ($Language -eq 'en') { 'LANG_ENGLISH' } else { 'LANG_POLISH' }
    # The STRINGTABLE that follows this LANGUAGE statement, up to its END.
    $pattern = "LANGUAGE\s+$langToken\s*,[^\r\n]*\r?\n\s*STRINGTABLE\s*\r?\nBEGIN\r?\n(?<body>.*?)\r?\nEND"
    $m = [regex]::Match($text, $pattern, 'Singleline')
    if (-not $m.Success) { throw "no STRINGTABLE found for $langToken in $RcPath" }

    $table = @{}
    foreach ($line in $m.Groups['body'].Value -split "`r?`n") {
        $entry = [regex]::Match($line, '^\s*(?<id>IDS_[A-Z0-9_]+)\s+"(?<val>.*)"\s*$')
        if (-not $entry.Success) { continue }
        $value = $entry.Groups['val'].Value -replace '\\r\\n', "`r`n" -replace '\\n', "`n"
        $table[$entry.Groups['id'].Value] = $value
    }
    if ($table.Count -eq 0) { throw "STRINGTABLE for $langToken parsed as empty" }
    return $table
}

function Expand-ResourceArg {
    <#  Apply the single %s substitution that i18n::load_string_arg performs. #>
    param([Parameter(Mandatory)][string]$Template, [Parameter(Mandatory)][string]$Value)
    $at = $Template.IndexOf('%s')
    if ($at -lt 0) { return $Template }
    return $Template.Substring(0, $at) + $Value + $Template.Substring($at + 2)
}

function Start-Installer {
    <#  Launch the release build and return the process plus its exe path. #>
    param([string]$ExePath)
    if (-not $ExePath) {
        $ExePath = Join-Path $PSScriptRoot '..\..\target\x86_64-pc-windows-msvc\release\pacenotes-installer.exe'
    }
    $ExePath = (Resolve-Path $ExePath).Path
    return [pscustomobject]@{
        Process = Start-Process $ExePath -PassThru
        Path    = $ExePath
    }
}

Export-ModuleMember -Function * -Variable *
