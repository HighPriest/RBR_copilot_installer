# installer/tests

The installer's scripted checks live here: the size gate, the version sync,
the WinHTTP test and the GUI tests. Rust unit tests do not: they sit beside
the module they test (`src/gamedir/tests.rs`, `src/tree/tests.rs`), and this
directory takes Rust tests only when they drive the public API across several
modules, named after the scenario they cover.

Run the lot:

```powershell
pwsh -File tests/Run-AllTests.ps1          # everything
pwsh -File tests/Run-AllTests.ps1 -SkipUi  # headless: logic + size gate only
```

## What is here

| Path | Kind | Needs a desktop? | In CI? |
|---|---|---|---|
| `cargo test` | AUTOMATED TEST — the unit tests in `src/` | no | yes |
| `Test-VersionSync.ps1` | AUTOMATED TEST — Cargo.toml vs .rc VERSIONINFO | no | yes |
| `Test-BinarySize.ps1` | AUTOMATED TEST — the 500 kB release gate | no | yes |
| `Test-HttpClient.ps1` | AUTOMATED TEST — WinHTTP vs a live server | no | yes |
| `ui/Test-UiLayout.ps1` | AUTOMATED TEST — walks the wizard, scenes 1-3 | yes | no |
| `ui/Get-UiScreenshot.ps1` | AGENT / DEVELOPER AID — captures PNGs of scenes 1-7 | yes | no |
| `ui/Win32Ui.psm1` | SUPPORT MODULE — shared user32.dll helpers | n/a | n/a |
| `Test-PublicEndpoint.ps1` | MANUAL CHECK — the real server over HTTPS | no | no |
| `ui/Test-WizardDownload.ps1` | AUTOMATED TEST — scenes 1-7, live catalogue, real downloads | yes | no |
| `Run-AllTests.ps1` | runner for all of the above, plus fmt + clippy | optional | no |

`Run-AllTests.ps1` also runs `cargo fmt --check` and `cargo clippy` under
`-D warnings`, matching the CI installer job. The installer is excluded from
the Cargo workspace, so the root `server` CI job's fmt/clippy never touch it —
without these it would go unlinted.

CI skips the GUI tests (they drive real dialogs and need an interactive
desktop session) and `Test-PublicEndpoint.ps1` (it depends on a machine
someone else runs).

`Test-PublicEndpoint.ps1` earns its place despite not being a gate: every
other test talks plain HTTP to loopback, so it is the ONLY one that takes the
`WINHTTP_FLAG_SECURE` branch and exercises the Schannel handshake. That path
is the justification for shipping no TLS crate at all, so it needs something
that can actually fail. Run it when the deployment changes.

Three kinds, and each script states which it is in its own `.SYNOPSIS`:

| Kind | Meaning | Gate CI on it? |
|---|---|---|
| **AUTOMATED TEST** | Asserts, hermetic, exits non-zero on failure. | Yes |
| **AGENT / DEVELOPER AID** | Produces artifacts for a person or an agent to look at. Asserts nothing, always exits 0. | No |
| **MANUAL CHECK** | Asserts, but against something OUTSIDE this repo. | No — a red result often says nothing about the code |

Anything that downloads is given a throwaway cache with `--cache`. Without
it a run would read back whatever a previous run left in the per-user cache —
so the suite would pass or fail depending on history — and would quietly fill
a developer's profile with fixture packages. `Get-UiScreenshot.ps1` takes the
same flag, for the same reason: a scene 6 with nothing to download is not
worth a screenshot.

`Test-HttpClient.ps1` builds and runs a real `pacenotes-server`: WinHTTP
cannot be unit tested, so the only honest test is against the server the
installer actually talks to. It drives the binary's `--selftest-http` flag,
which writes a report to a file rather than opening a window — which is why
it runs in CI while the GUI suite does not.

`cargo test` ignores `ui/` and the
`.ps1` files, so the two suites coexist in one directory without interfering.

## How tests here are written

Two rules, both learned by writing worse versions first.

### 1. No arbitrary inputs

A test input must stand for a real situation the user can be in. The
game-folder field exists to tell *"this is your Richard Burns Rally install"*
from *"this is some other folder"*, so its fixtures are exactly that: a
directory containing `RichardBurnsRally_SSE.exe`, and one that is not. Handing
it `$env:TEMP` would assert nothing about the thing the field is for — and
indeed an earlier version of this suite did that, and happily passed while the
code accepted **any** directory that existed.

Likewise, the "Install proceeds once all three categories have a pick" path
was deliberately **not** asserted while those lists were empty: injecting
placeholder rows would only have proved that a listbox can hold a string. It
is asserted now, in `ui/Test-WizardDownload.ps1`, because scene 5 fills the
lists from a real catalogue served by a real server — and the rows it checks
for are counted from that catalogue rather than written out a second time.

### 2. Expected text comes from the resource, not from a copy

`ui/Test-UiLayout.ps1` reads `res/installer.rc` via `Get-ResourceStrings` and
asserts each control shows exactly what the resource defines for the language
under test. It runs the wizard against a deliberately dead server
(`--server http://127.0.0.1:1`) so scene 2 takes its documented OFFLINE path
and the walk is deterministic; the online paths belong to
`Test-HttpClient.ps1`. Hardcoding the strings would only prove that two hand-written
copies agree, and would keep passing if a caption were bound to the wrong id or
a translation were never added.

The one deliberate exception is the language picker, which is shown *before* a
language has been chosen and therefore renders in the OS UI language. It cannot
honor a choice the user has not made, so its captions are asserted to be a
shipped string for their id in *some* language — which still catches the `<id>`
fallback that `i18n::load_string` emits for a missing or mis-bound id.

## Traps these scripts encode

Do not "simplify" these away; each cost real debugging time and the workaround
is commented at its call site.

1. **`GetWindowText` does not cross process boundaries.** For a window owned by
   another process it returns only the cached window title. Static and Button
   captions live in that cache and read back fine, but Edit and ListBox
   contents come back empty — silently turning assertions into false passes.
   `Get-CtrlText` uses `WM_GETTEXT`, which USER32 does marshal.

2. **`TCM_SETCURSEL` does not notify the parent.** Setting a tab selection that
   way skips the app's `TCN_SELCHANGE` handler, so the test proves nothing. A
   synthetic `WM_NOTIFY` is not an option either — it needs an `NMHDR` in the
   target process's address space. `Select-Tab` sends arrow keys, so the tab
   control raises a genuine `TCN_SELCHANGE`.

3. **PowerShell `-like` treats `[...]` as a character class.** Log lines are
   prefixed with `[Game Name]`, so `-like "*[BeamNG]*"` matches almost any
   text. Use `.Contains()` for those.

4. **PowerShell variable names are case-insensitive.** A local `$tab` silently
   clobbers a parameter `$Tab`. Helper parameters here are named distinctly.

5. **`SetForegroundWindow` is refused to a process that is not already in the
   foreground**, so a screen grab silently captures whatever IS on top — this
   produced screenshots of unrelated applications. `Get-UiScreenshot.ps1` uses
   `PrintWindow` with `PW_RENDERFULLCONTENT`, which asks the window to render
   itself and ignores z-order.

6. **Wizard page controls are GRANDchildren of the property sheet.** The sheet
   owns one child dialog per page and only the active one is visible, so
   `GetDlgItem` on the sheet finds none of them. `Get-ActivePage` finds the
   visible page first; a test that skipped it would conclude every control was
   missing.

7. **A MessageBox's OK button is not reliably control id 1.** On this app's
   own warning boxes it came back as id 2, so the posted `WM_COMMAND` IDOK
   that `Dismiss-Modal` used to send closed nothing. A box left standing is
   particularly nasty here: the page underneath is disabled, yet its child
   controls still answer messages sent straight to them, so the symptom is a
   click that "does nothing" several steps later. `Dismiss-Modal` now clicks
   whichever button the box actually has.

8. **Tree and list selections must be made by CLICK, not by key.** Sending
   `VK_SPACE` to a checkbox tree from outside the process toggled nothing,
   and `VK_HOME`/`VK_DOWN` moved a listbox selection without the parent ever
   getting `LBN_SELCHANGE` — so the app never learned what was picked. A
   `WM_LBUTTONDOWN` on the row is handled from the message alone and raises
   the real notification. `Invoke-TreeCheckRoots` and `Select-ListItem` take
   the geometry from the control (item height, indent) rather than assuming
   it.

## Requirements

- The GUI tests are **Windows-only** and need an **interactive desktop
  session**; they will not run on a headless agent. `cargo test` and the size
  gate have no such restriction.
- Build before running the GUI tests: they default to
  `target/x86_64-pc-windows-msvc/release/pacenotes-installer.exe`. Pass
  `-ExePath` to point elsewhere.
- Scripts kill the installer in a `finally` block and delete their fixtures, so
  a failed run leaves no stray window or temp folder behind.
