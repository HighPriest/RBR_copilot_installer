# installer/tests

Two different kinds of thing live here. Check which kind a script is before
relying on it — the distinction is deliberate and is stated in each script's
`.SYNOPSIS` header.

| Kind | Meaning | Gate CI on it? |
|---|---|---|
| **AUTOMATED TEST** | Asserts, prints PASS/FAIL, exits non-zero on failure. | Yes |
| **AGENT / DEVELOPER AID** | Produces artifacts for a human or AI agent to inspect. Asserts nothing, always exits 0. | No |
| **SUPPORT MODULE** | Shared helpers. Not runnable on its own. | n/a |

## Contents

### `ui/Win32Ui.psm1` — SUPPORT MODULE

Shared `user32.dll` helpers: enumerate dialogs and controls, read/write control
text, drive tab controls, launch the binary. Imported by both scripts below.

### `ui/Test-UiLayout.ps1` — AUTOMATED TEST

Drives the real release `.exe` and asserts the dialog layout and behavior match
[`../DESIGN.md`](../DESIGN.md): control inventory, the three tab strips, page
show/hide, localization reaching real controls, game-folder validation, the
Install preconditions, the error → Logs auto-switch, per-game state retention,
and clean shutdown. 55 checks; exits 1 if any fail.

```powershell
pwsh -File tests/ui/Test-UiLayout.ps1            # English
pwsh -File tests/ui/Test-UiLayout.ps1 -Language 1  # Polish
```

Run it in **both** languages. The Polish pass is what proves the per-language
STRINGTABLE lookup works and that UTF-8 diacritics survive into real controls.

### `ui/Get-UiScreenshot.ps1` — AGENT / DEVELOPER AID

Captures PNGs of the dialogs. `Test-UiLayout.ps1` proves a control exists and
holds the right text; it cannot tell you a label is clipped, two buttons
overlap, or a panel has a dead area. This closes that gap — run it, then look
at (or, as an agent, `Read`) the images.

```powershell
pwsh -File tests/ui/Get-UiScreenshot.ps1
pwsh -File tests/ui/Get-UiScreenshot.ps1 -Language 1 -GameTab 1 -RightTab 1 -InfoTab 1
```

Screenshots go to `$env:TEMP` by default — deliberately not the repo, since
they are throwaway artifacts. Pass `-OutDir` to change that.

## Requirements and caveats

- **Windows only**, and these need an **interactive desktop session**: they
  launch real windows and read them back. They will not work on a headless
  agent. The pure-logic Rust tests (`cargo test --lib`) have no such
  restriction and are the cross-platform half of the suite.
- Build first: both scripts default to
  `target/x86_64-pc-windows-msvc/release/pacenotes-installer.exe`. Pass
  `-ExePath` to point somewhere else.
- The scripts kill the process in a `finally` block, so a failed run does not
  leave a stray window behind.

## Two traps these scripts encode

Both cost real debugging time; the workarounds are commented at their call
sites so they do not get "simplified" away.

1. **`GetWindowText` does not cross process boundaries.** For a window owned by
   another process it returns only the cached window title. Static and Button
   captions live in that cache and read back fine, but Edit and ListBox
   contents come back as empty strings — which silently turns assertions into
   false passes. `Get-CtrlText` uses `WM_GETTEXT`, which USER32 does marshal.

2. **`TCM_SETCURSEL` does not notify the parent.** Setting a tab selection that
   way would skip the app's `TCN_SELCHANGE` handler entirely, so the test would
   prove nothing. A synthetic `WM_NOTIFY` is not an option either — it needs an
   `NMHDR` in the target process's address space. `Select-Tab` therefore sends
   arrow keys, which make the tab control raise a genuine `TCN_SELCHANGE`.

A third trap worth knowing when editing these: **PowerShell variable names are
case-insensitive**, so a local `$tab` will silently clobber a parameter `$Tab`.
Helper parameters here are named distinctly to avoid it.
