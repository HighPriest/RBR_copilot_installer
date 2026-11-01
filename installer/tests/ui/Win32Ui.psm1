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
  [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr h);
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
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern IntPtr SetFocus(IntPtr h);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
}
"@

# Keys through the dialog manager, for what only it decides: Tab order, what
# Enter presses, where focus starts. A message SENT to a control skips it; one
# POSTED to the wizard's queue goes through the loop that calls it (Send-Key).
Add-Type @"
using System;
using System.Runtime.InteropServices;

public static class NativeKeys {
  [StructLayout(LayoutKind.Sequential)] public struct GUITHREADINFO {
    public int cbSize; public int flags; public IntPtr hwndActive; public IntPtr hwndFocus;
    public IntPtr hwndCapture; public IntPtr hwndMenuOwner; public IntPtr hwndMoveSize;
    public IntPtr hwndCaret; public int caretLeft, caretTop, caretRight, caretBottom; }
  [DllImport("user32.dll")] public static extern bool GetGUIThreadInfo(uint tid, ref GUITHREADINFO i);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  // Another process's focus: GetFocus only answers for the calling thread.
  public static IntPtr Focus(IntPtr window) {
    uint pid; uint tid = GetWindowThreadProcessId(window, out pid);
    var info = new GUITHREADINFO(); info.cbSize = Marshal.SizeOf(info);
    return GetGUIThreadInfo(tid, ref info) ? info.hwndFocus : IntPtr.Zero; }
}
"@

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
$script:LB_GETCURSEL = 0x0188
$script:LB_GETCOUNT = 0x018B
$script:LB_GETITEMHEIGHT = 0x01A1
$script:LB_SETTOPINDEX = 0x0197
$script:TCM_GETITEMCOUNT = 0x1304
$script:TCM_GETCURSEL = 0x130B
# Tree view: only the HTREEITEM-based messages are usable cross-process.
$script:TVM_GETNEXTITEM = 0x110A
$script:TVM_SELECTITEM = 0x110B
$script:TVM_GETCOUNT = 0x1105
$script:TVM_GETITEMHEIGHT = 0x111C
$script:TVM_GETINDENT = 0x1106
$script:TVM_GETITEMSTATE = 0x1127
$script:TVIS_STATEIMAGEMASK = 0xF000
$script:TVGN_ROOT = 0
$script:TVGN_NEXT = 1
$script:TVGN_CARET = 9
$script:WM_LBUTTONDOWN = 0x0201
$script:WM_LBUTTONUP = 0x0202
$script:MK_LBUTTON = 1
$script:BM_CLICK = 0x00F5
$script:VK_RIGHT = 0x27
$script:VK_LEFT = 0x25
$script:VK_HOME = 0x24
$script:VK_DOWN = 0x28

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
    <#  Synthesize BN_CLICKED (HIWORD 0) for a control id.

        -Post for a click that opens a modal: a SENT message returns only when
        the handler does, and the handler is inside the modal's loop until
        someone closes it — which the test, still waiting, never will. #>
    param([Parameter(Mandatory)][IntPtr]$Dialog, [Parameter(Mandatory)][int]$CtrlId, [switch]$Post)
    if ($Post) {
        [void][NativeUi]::PostMessageW($Dialog, $script:WM_COMMAND, [IntPtr]$CtrlId, [IntPtr]::Zero)
    } else {
        [void][NativeUi]::SendMessageW($Dialog, $script:WM_COMMAND, [IntPtr]$CtrlId, [IntPtr]::Zero)
    }
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

function Get-ListCount {
    param([Parameter(Mandatory)][IntPtr]$Dialog, [Parameter(Mandatory)][int]$CtrlId)
    $h = [NativeUi]::GetDlgItem($Dialog, $CtrlId)
    return [int][NativeUi]::SendMessageW($h, $script:LB_GETCOUNT, [IntPtr]::Zero, [IntPtr]::Zero)
}

function Get-ListSelection {
    <#  -1 when nothing is selected (LB_ERR). #>
    param([Parameter(Mandatory)][IntPtr]$Dialog, [Parameter(Mandatory)][int]$CtrlId)
    $h = [NativeUi]::GetDlgItem($Dialog, $CtrlId)
    return [int][NativeUi]::SendMessageW($h, $script:LB_GETCURSEL, [IntPtr]::Zero, [IntPtr]::Zero)
}

function Select-ListItem {
    <#  Pick a listbox item by CLICKING it.

        Not LB_SETCURSEL: that changes the selection without notifying the
        parent, so the app's LBN_SELCHANGE handler never runs and the model
        never learns what was picked — the assertion would pass while the pick
        went nowhere (the same trap as Select-Tab). Not keys either: Home/Down
        sent to the control do move the selection, measured, but the parent
        gets no LBN_SELCHANGE, so the app still saw nothing chosen. A click
        on the row is handled from the message alone and notifies.

        The row is located from the control's own item height, and the list is
        scrolled to the top first so index and row coincide. Item TEXT cannot
        be read cross-process at all (LB_GETTEXT wants a buffer in the
        target's address space and USER32 does not marshal it), which is why
        callers assert on counts and selection indices. #>
    param(
        [Parameter(Mandatory)][IntPtr]$Dialog,
        [Parameter(Mandatory)][int]$CtrlId,
        [Parameter(Mandatory)][int]$Index
    )
    $h = [NativeUi]::GetDlgItem($Dialog, $CtrlId)
    [void][NativeUi]::SendMessageW($h, $script:LB_SETTOPINDEX, [IntPtr]0, [IntPtr]::Zero)
    $height = [int][NativeUi]::SendMessageW($h, $script:LB_GETITEMHEIGHT, [IntPtr]0, [IntPtr]::Zero)
    $point = [IntPtr]((($Index * $height + [int]($height / 2)) -shl 16) -bor 8)
    [void][NativeUi]::SendMessageW($h, $script:WM_LBUTTONDOWN, [IntPtr]$script:MK_LBUTTON, $point)
    [void][NativeUi]::SendMessageW($h, $script:WM_LBUTTONUP, [IntPtr]0, $point)
    Start-Sleep -Milliseconds 150
    return (Get-ListSelection -Dialog $Dialog -CtrlId $CtrlId)
}

function Get-ControlFill {
    <#  The colour a control is painted in, as a person sees it.

        The whole top-level window renders itself through PrintWindow with
        PW_RENDERFULLCONTENT, as Get-UiScreenshot.ps1 does, so the wizard
        need not be in front. The sample sits 6 px in from the control's left
        edge and halfway down: past a two-pixel focus border, and clear of a
        centred label. #>
    param([Parameter(Mandatory)][IntPtr]$Window, [Parameter(Mandatory)][IntPtr]$Control)
    Add-Type -AssemblyName System.Drawing
    $outer = New-Object RECT; $inner = New-Object RECT
    [void][NativeUi]::GetWindowRect($Window, [ref]$outer)
    [void][NativeUi]::GetWindowRect($Control, [ref]$inner)
    $bitmap = New-Object System.Drawing.Bitmap ($outer.R - $outer.L), ($outer.B - $outer.T)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $hdc = $graphics.GetHdc()
    [void][NativeUi]::PrintWindow($Window, $hdc, 2)
    $graphics.ReleaseHdc($hdc); $graphics.Dispose()
    $color = $bitmap.GetPixel($inner.L - $outer.L + 6, [int](($inner.T + $inner.B) / 2) - $outer.T)
    $bitmap.Dispose()
    return $color
}

function Test-IsGreen {
    <#  Green, not just light: the theme's buttons are grey, R = G = B. #>
    param([Parameter(Mandatory)]$Color)
    return ($Color.G - [Math]::Max($Color.R, $Color.B)) -ge 20
}

function Get-TreeRowCount {
    param([Parameter(Mandatory)][IntPtr]$Dialog, [Parameter(Mandatory)][int]$CtrlId)
    $h = [NativeUi]::GetDlgItem($Dialog, $CtrlId)
    return [int][NativeUi]::SendMessageW($h, $script:TVM_GETCOUNT, [IntPtr]::Zero, [IntPtr]::Zero)
}

function Get-TreeStateImage {
    <#  The item's state-image index: 1 unchecked, 2 checked, 3 partial.

        TVM_GETITEMSTATE is one of the few tree messages usable from another
        process — it takes an HTREEITEM and a mask and returns a plain value,
        with no TVITEM to place in the target's address space. #>
    param([Parameter(Mandatory)][IntPtr]$Tree, [Parameter(Mandatory)][IntPtr]$Item)
    $state = [int][NativeUi]::SendMessageW($Tree, $script:TVM_GETITEMSTATE, $Item, [IntPtr]$script:TVIS_STATEIMAGEMASK)
    return (($state -band $script:TVIS_STATEIMAGEMASK) -shr 12)
}

function Invoke-TreeCheckRoots {
    <#  Tick every top-level row of a checkbox tree, the way a user would.

        CLICKS, not keys. VK_SPACE sent to the control toggles nothing from
        outside the process — measured: all four rows stayed on state image 1,
        and once a single unrelated row flipped instead. A WM_LBUTTONDOWN on
        the state icon is handled from the message alone, raises the real
        TVN_ITEMCHANGED, and so exercises the app's own propagation. Writing
        the state image with TVM_SETITEM would not: it needs a TVITEM in the
        target's address space, and it would skip the notification that is the
        whole point.

        The geometry is only valid for TOP-LEVEL rows with every row
        collapsed, which is how this tree opens: row i then sits at
        y = i * itemheight, and a root's state icon sits just past the
        expand button, i.e. one indent in. Both measurements come from the
        control (TVM_GETITEMHEIGHT, TVM_GETINDENT) rather than being assumed.

        Returns the number of roots that ended up ticked. #>
    param([Parameter(Mandatory)][IntPtr]$Dialog, [Parameter(Mandatory)][int]$CtrlId)
    $h = [NativeUi]::GetDlgItem($Dialog, $CtrlId)
    $height = [int][NativeUi]::SendMessageW($h, $script:TVM_GETITEMHEIGHT, [IntPtr]::Zero, [IntPtr]::Zero)
    $indent = [int][NativeUi]::SendMessageW($h, $script:TVM_GETINDENT, [IntPtr]::Zero, [IntPtr]::Zero)

    $roots = @()
    $item = [NativeUi]::SendMessageW($h, $script:TVM_GETNEXTITEM, [IntPtr]$script:TVGN_ROOT, [IntPtr]::Zero)
    while ($item -ne [IntPtr]::Zero -and $roots.Count -lt 64) {
        $roots += $item
        $item = [NativeUi]::SendMessageW($h, $script:TVM_GETNEXTITEM, [IntPtr]$script:TVGN_NEXT, $item)
    }

    for ($row = 0; $row -lt $roots.Count; $row++) {
        # A click toggles and cached rows start ticked, so only unticked rows are clicked.
        if ((Get-TreeStateImage -Tree $h -Item $roots[$row]) -eq 2) { continue }
        $x = $indent + [int]($height / 2)
        $y = $row * $height + [int]($height / 2)
        $point = [IntPtr](($y -shl 16) -bor $x)
        [void][NativeUi]::SendMessageW($h, $script:WM_LBUTTONDOWN, [IntPtr]$script:MK_LBUTTON, $point)
        [void][NativeUi]::SendMessageW($h, $script:WM_LBUTTONUP, [IntPtr]0, $point)
        Start-Sleep -Milliseconds 150
    }
    Start-Sleep -Milliseconds 250
    return ($roots | Where-Object { (Get-TreeStateImage -Tree $h -Item $_) -eq 2 }).Count
}

function Close-Dialog {
    param([Parameter(Mandatory)][IntPtr]$Dialog)
    [void][NativeUi]::PostMessageW($Dialog, $script:WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero)
}

# comctl32 button ids from prsht.h, and the PSM_PRESSBUTTON indices.
$script:ID_WIZBACK = 0x3023
$script:ID_WIZNEXT = 0x3024
$script:ID_WIZFINISH = 0x3025
$script:PSM_PRESSBUTTON = 1137   # WM_USER + 113
$script:PSBTN_BACK = 0
$script:PSBTN_NEXT = 1
$script:PSBTN_CANCEL = 5

function Get-ActivePage {
    <#
    .SYNOPSIS
        The wizard page currently showing inside a property sheet.

    .DESCRIPTION
        Page controls are GRANDchildren of the sheet: the sheet owns a child
        dialog per page and only one is visible. GetDlgItem on the sheet
        therefore cannot see them, and a test that tried would conclude every
        control was missing.
    #>
    param([Parameter(Mandatory)][IntPtr]$Sheet)
    $script:page = [IntPtr]::Zero
    $cb = [NativeUi+EnumProc] {
        param($h, $l)
        if ([NativeUi]::IsWindowVisible($h)) {
            $c = New-Object System.Text.StringBuilder 64
            [void][NativeUi]::GetClassNameW($h, $c, 64)
            if ($c.ToString() -eq '#32770') { $script:page = $h; return $false }
        }
        return $true
    }
    [void][NativeUi]::EnumChildWindows($Sheet, $cb, [IntPtr]::Zero)
    return $script:page
}

function Wait-ForPageControl {
    <#  Poll until the active page exposes $CtrlId, i.e. the next scene has
        actually been built. Page switching is asynchronous. #>
    param(
        [Parameter(Mandatory)][IntPtr]$Sheet,
        [Parameter(Mandatory)][int]$CtrlId,
        [int]$TimeoutMs = 5000
    )
    $deadline = (Get-Date).AddMilliseconds($TimeoutMs)
    while ((Get-Date) -lt $deadline) {
        $page = Get-ActivePage -Sheet $Sheet
        if ($page -ne [IntPtr]::Zero -and
            [NativeUi]::GetDlgItem($page, $CtrlId) -ne [IntPtr]::Zero) {
            return $page
        }
        Start-Sleep -Milliseconds 100
    }
    return [IntPtr]::Zero
}

function Get-WizardButtonText {
    param([Parameter(Mandatory)][IntPtr]$Sheet, [Parameter(Mandatory)][int]$ButtonId)
    $h = [NativeUi]::GetDlgItem($Sheet, $ButtonId)
    if ($h -eq [IntPtr]::Zero) { return $null }
    $cap = 512
    $sb = New-Object System.Text.StringBuilder $cap
    [void][NativeUi]::SendMessageGetText($h, 0x000D, [IntPtr]$cap, $sb)
    return $sb.ToString()
}

function Test-WizardButtonEnabled {
    param([Parameter(Mandatory)][IntPtr]$Sheet, [Parameter(Mandatory)][int]$ButtonId)
    return [NativeUi]::IsWindowEnabled([NativeUi]::GetDlgItem($Sheet, $ButtonId))
}

function Press-WizardButton {
    <#  PSM_PRESSBUTTON drives the sheet exactly as a click would, including
        the PSN_WIZNEXT round trip to the page — which is the part worth
        testing. Posting WM_COMMAND to the button would skip it. #>
    param([Parameter(Mandatory)][IntPtr]$Sheet, [Parameter(Mandatory)][int]$Button)
    [void][NativeUi]::PostMessageW($Sheet, $script:PSM_PRESSBUTTON, [IntPtr]$Button, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}

function Dismiss-Modal {
    <#  Close a MessageBox the app raised, by clicking the button it actually
        has.

        NOT a posted WM_COMMAND with IDOK: the single OK button of a
        MessageBox is not reliably control id 1. Measured on this app's own
        warning boxes, the OK button came back as id 2, so posting IDOK left
        the box standing — and a box left standing is poison for a UI test,
        because the page underneath stays disabled while its child controls
        still answer messages directly. That reads as "the click did nothing"
        several steps later, nowhere near the cause.

        BM_CLICK goes to the button itself, which then raises whatever command
        id it owns. Posted, not sent: the app's thread is inside the modal
        loop and a send would deadlock on a busy box. #>
    param([Parameter(Mandatory)][IntPtr]$Modal)
    $button = Get-ControlInventory -Dialog $Modal | Where-Object { $_.Class -eq 'Button' } | Select-Object -First 1
    if ($button) {
        [void][NativeUi]::PostMessageW($button.Handle, $script:BM_CLICK, [IntPtr]0, [IntPtr]::Zero)
    }
    else {
        # No button to click (should not happen for MessageBox); ask the box
        # to close itself.
        [void][NativeUi]::PostMessageW($Modal, $script:WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero)
    }
    Start-Sleep -Milliseconds 400
}

function Get-FocusedControlId {
    <#  The control id that has the keyboard focus in the window's process. #>
    param([Parameter(Mandatory)][IntPtr]$Window)
    $focused = [NativeKeys]::Focus($Window)
    if ($focused -eq [IntPtr]::Zero) { return 0 }
    return [NativeUi]::GetDlgCtrlID($focused)
}

function Send-Key {
    <#  Press and release one key on the control that has the focus.

        POSTED into the wizard's queue, not synthesized as input: the property
        sheet's message loop takes it out and hands it to the dialog manager
        exactly as it does a real key, so Tab order, the default button and a
        control's own key handling all still decide. Real input (keybd_event)
        goes to whatever window is in FRONT, and Windows' foreground lock
        would not reliably let a background test bring the wizard there —
        measured: it lost a key, and then refused outright — which risks
        typing into another application on the desktop. #>
    param([Parameter(Mandatory)][IntPtr]$Window, [Parameter(Mandatory)][byte]$VirtualKey)
    $target = [NativeKeys]::Focus($Window)
    if ($target -eq [IntPtr]::Zero) { $target = $Window }
    # lParam: repeat count 1; the key-up also sets the previous-state and transition bits.
    [void][NativeUi]::PostMessageW($target, $script:WM_KEYDOWN, [IntPtr]$VirtualKey, [IntPtr]1)
    [void][NativeUi]::PostMessageW($target, $script:WM_KEYUP, [IntPtr]$VirtualKey, [IntPtr]0xC0000001)
    Start-Sleep -Milliseconds 400
}

function Move-Focus {
    <#  Focus a control the way the dialog manager does (WM_NEXTDLGCTL), which
        SetFocus cannot do across processes. #>
    param([Parameter(Mandatory)][IntPtr]$Sheet, [Parameter(Mandatory)][IntPtr]$Control)
    [void][NativeUi]::PostMessageW($Sheet, 0x0028, $Control, [IntPtr]1)
    Start-Sleep -Milliseconds 300
}

function Decline-Modal {
    <#  Answer No on a Yes/No MessageBox, by clicking the button whose id is
        IDNO (7).

        Not Dismiss-Modal, which clicks the FIRST button: on a Yes/No box that
        is Yes, and saying yes to the installer's restart-as-administrator
        question would start a real UAC prompt in the middle of a test. With no
        No button, this throws rather than guess. #>
    param([Parameter(Mandatory)][IntPtr]$Modal)
    $no = Get-ControlInventory -Dialog $Modal | Where-Object { $_.Class -eq 'Button' -and $_.Id -eq 7 }
    if (-not $no) { throw 'the box has no No button' }
    [void][NativeUi]::PostMessageW($no.Handle, $script:BM_CLICK, [IntPtr]0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 400
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
    <#  Launch the release build and return the process plus its exe path.

        Refuses while another copy runs, whose single-instance modal would
        otherwise surface as "scene 1 never appeared". #>
    param([string]$ExePath, [string[]]$ExtraArgs = @())
    if (-not $ExePath) {
        $ExePath = Join-Path $PSScriptRoot '..\..\target\x86_64-pc-windows-msvc\release\pacenotes-installer.exe'
    }
    $ExePath = (Resolve-Path $ExePath).Path
    $running = @(Get-Process -Name 'pacenotes-installer' -ErrorAction SilentlyContinue)
    if ($running.Count -gt 0) {
        throw ("pacenotes-installer is already running (pid $($running.Id -join ', ')). " +
            'The installer allows one instance, so this run would only see the ' +
            '"already running" modal. Close it and try again.')
    }
    $proc = if ($ExtraArgs.Count -gt 0) {
        Start-Process $ExePath -ArgumentList $ExtraArgs -PassThru
    } else {
        Start-Process $ExePath -PassThru
    }
    return [pscustomobject]@{ Process = $proc; Path = $ExePath }
}

Export-ModuleMember -Function * -Variable *
