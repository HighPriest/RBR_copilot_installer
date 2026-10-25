# installer/CLAUDE.md — Windows GUI installer

Windows-only (x86_64-pc-windows-msvc). HARD RELEASE GATE: final .exe < 500 kB.
Run `tests/Test-BinarySize.ps1` after every change that touches dependencies or adds
noticeable code. If size regresses, fix it before continuing.

## Why the binary stays small — do not break these

1. **HTTPS = WinHTTP** (`Win32_Networking_WinHttp`). The OS provides TLS
   (Schannel). Never add reqwest/hyper/rustls/native-tls/openssl.
   VERIFIED against the real endpoint: handshake, chain and SNI all work
   through WinHTTP. Keep it verified — every other test talks plain HTTP to
   loopback, so `tests/Test-PublicEndpoint.ps1` is the ONLY thing that takes
   the `WINHTTP_FLAG_SECURE` branch. Run it when the deployment changes.
2. **UI = raw Win32 dialogs** from `res/installer.rc` templates, compiled by
   `embed-resource` in build.rs, driven by `DialogBoxParamW`/`CreateDialogParamW`
   and a dlgproc. Never add a GUI framework crate.
3. **Zip = miniz_oxide** + a hand-rolled zip central-directory parser in
   `src/zip.rs` (~150 lines). Do not add the `zip` crate.
4. **No tokio.** Concurrency = one `std::thread` worker + `PostMessageW` to
   the UI thread (custom `WM_APP+n` messages carry progress %, status string
   index, completion). This replaces the C# BackgroundWorker pattern.
5. serde_json is allowed but on notice (also used for cache.json + ledger):
   if the final binary is over budget, the fallback is a minimal hand-rolled
   JSON parser/writer. Keep all JSON usage isolated in `api.rs` + `cache.rs`
   + `ledger.rs` so the swap stays cheap.
6. Prefer `windows-sys` (thin declarations) over `windows` (heavy).
7. No TOML in the installer: sidecar metadata is server-side only; the
   installer sees it pre-merged in the manifest JSON.

## Install model (see root CLAUDE.md for rationale)

- Packages are game-root overlay zips. ONE install mechanism: extract into
  game dir, record files in the ledger. NEVER interpret or execute anything
  found inside a package.
- **A package is identified by `game` + `directory` + `name`.** One server
  serves every game, so `"directory/name"` is NOT unique: two games can both
  hold `public/style/Numeric Standard`. Every key the installer stores — the
  cache index, the ledger, any ref it writes down — must carry the game, or
  one game's package silently shadows another's. `PackageInfo::download_path()`
  produces exactly that triple, `game/directory/name`, and is also the tail of
  the download URL; prefer it over assembling keys by hand.
- **Cache** (`cache.rs`): `%LOCALAPPDATA%\PacenotesInstaller\cache\`
  - `<sha256>.zip` — the payloads, keyed by manifest sha (integrity+dedupe)
  - `cache.json` — index: (game, directory, name) → {sha256, version, size,
    updated_at, display_name, exclusive_group, requires} — a snapshot of
    PackageInfo, so the Download tree works fully OFFLINE. Category and the
    tree levels are NOT stored: they are derived from `name`.
  - Download = fetch → sha256 verify → move into cache. Re-download only when
    manifest sha differs from cached sha.
- **Ledger** (`ledger.rs`): per game dir, `.pacenotes\installed.json`.
  `.pacenotes\` rather than somewhere in the plugin tree, because that path
  is RBR-only and is written to by other tools. Written atomically
  (temp + rename) AFTER each successful package install/uninstall, not once
  at the end.

      "<game>/<directory>/<name>": {
        "sha256": …, "version": …, "exclusive_group": …,
        "state": "installed" | "installing",
        "files": {
          "plugins/pacenote/pacenote.ini": {        // key: case-folded
            "path": "Plugins/Pacenote/PaceNote.ini", // as actually written
            "sha256": …,                             // what WE wrote
            "prior": "backup" | "identical" | "absent"
          }
        }
      }

  Every part of that shape pays for itself, and a plain list of paths
  cannot do any of it:
  - **case-folded keys**, original spelling kept. NTFS ignores case, so
    `Plugins/Pacenote` and `plugins/pacenote` are one file. Comparing keys
    byte-for-byte answers "does another package still own this?" wrongly,
    and the wrong answer deletes a file another package needs.
  - **per-file `sha256`** of what we wrote. Steam's verify, the RSF
    installer, a plugin update or the user's own editor can replace a file
    after we installed it. Uninstall deletes a file only when it still
    matches; otherwise it is left alone and logged. Someone else's work is
    not ours to remove.
  - **`prior`** — what was there before us. `backup`: the original is in a
    restore point. `identical`: the bytes already matched, so nothing was
    copied, and the original can be recovered from the cached payload.
    `absent`: there was no file, so uninstall deletes it. Without this, the
    byte-identical skip quietly turns files the user already had into files
    the installer owns and later deletes.
  - **`state: "installing"`**, written BEFORE extracting. A run that dies
    mid-extract leaves half-written files that are in no ledger entry — so
    the next run would treat them as the user's own and back them up as the
    pristine originals. The write-ahead entry makes them recognisably ours.
  - Paths are `/`-separated, as `zip::entries` returns them: a ledger must
    read the same on any machine.
- **Backup / restore points**: a run that replaces anything writes a dated
  restore point, `.pacenotes\restore\<timestamp>\` holding the files it
  replaced plus `ledger.json` as it was BEFORE the run. A file whose bytes
  already match what will replace it is not copied — nothing to preserve,
  and copying would duplicate hundreds of MB on a first run over an
  existing manual install; it is recorded as `prior: "identical"` instead.
  Restoring to a point is not just copying files back: it also removes what
  was installed after it, which is what the ledger snapshot is for. See
  `DESIGN.md` "Restore".
- **Install** (`engine.rs`): input = the wizard's result (the three
  Configuration picks plus the checked `pacenotes` packages) + the ledger.
  Plan = uninstall (ledgered but no longer chosen, including a ledgered
  member of a category or `exclusive_group` that a new pick supersedes) then
  install (chosen but not ledgered, or sha changed = update). Enforce
  `requires`. Scene 6 runs the plan and is the only progress screen.
- **Overlapping files** (two selected packages writing the same path):
  install order decides, the later one wins, and the run log says so. No
  gate and no refusal — a user must never be blocked by a catalogue mistake
  they cannot fix. The cost is accepted and known: after uninstalling the
  winner the file survives (another entry still owns it) holding the
  winner's bytes, so the folder drifts from what any one package expects.
  That is why the conflict is REPORTED to the server (`POST /report/conflict`)
  rather than only logged: the operator is the only one who can fix it, and
  they cannot fix what they never hear about.
- **Exclusivity** is enforced in the ENGINE only. A choice category is an
  implicit group, handled by scene 5 asking for one pick. For
  `exclusive_group`, `plan()` uninstalls the ledgered member before
  installing the new one. Neither scene 4 nor scene 5 gates it: two members
  ticked in one run both install, the later wins, and the overlap rule
  above reports it.
- **Choice categories are per game, and they live in `gamedir.rs`** — not in
  `shared`, where one list drove scene 5 for all four games, and not in the
  manifest. Each game declares which categories it asks a single pick from,
  with a localized LABEL and EXPLANATION per category: that text belongs in
  the `.rc` string tables with every other string, and shipping UI text
  through the manifest would mean the server deciding what the installer
  says, in languages it cannot know. These lists change rarely; an
  installer release for a new game is the right cost.
  - Only a game's choice categories are MANDATORY on scene 4. An operator
    adding an `extras/` folder must not turn it into a question every user
    has to answer.
  - A game with no choice categories skips scene 5 entirely.
- **Minimum manifest version.** Because the installer, not the server,
  knows the categories, a server that grows new ones is ahead of an old
  installer. So `Manifest` carries a version, every installer declares the
  highest it understands, and a manifest newer than that means the build is
  out of date. The rule for what happens then matters as much as the check:
  - It blocks DOWNLOADING only. An out-of-date installer may still
    reconfigure, repair and uninstall what is already on the machine — the
    user did not break anything by not updating, and taking their existing
    setup away from them would be the worst possible response.
  - The prompt appears when the user tries to download, not at startup.
  See S1 in `DESIGN-REVIEW.md`; this is the part of it decided now.

## Module map

- `main.rs` — entry point, common-controls init, the developer flags
  (`--server`, `--cache`), launches the wizard. `--cache` points a run at a
  throwaway cache, so the UI suite downloads for real.
- `selftest/` — `--selftest-http`, the WinHTTP check that
  `tests/Test-HttpClient.ps1` drives, and `--selftest-download`.
- `package_download.rs` — fetch a package into the cache, verified by sha256.
- `wizard/` — the UI. Layout per `DESIGN.md`: a comctl32 PropertySheet in
  wizard mode (`sheet.rs`), one page per scene (`scene/`), with `WizardState`
  (`state.rs`) shared across pages and what every page does alike in `page/`.
  Control ids mirror `res/installer.rc` in `control_ids.rs`. All seven scenes
  exist; scene 6 runs only the download phase until `zip.rs` (I6) and
  `engine.rs` (I7) land, and its log and summary say so. Before changing the
  sheet, read Implementation notes in `DESIGN.md`: the scene title, the
  per-scene button labels and the title bar all work around something the OS
  does not give.
- `ui/` — the small Win32 vocabulary the pages are written in: control
  text, combo boxes, list boxes, the three modal shapes, the folder picker.
- `choices.rs` — scene 5's model: the three single-select lists and the
  no-choice extras, derived from what scene 4 committed. Pure.
- `tree/` — scene 4's model: manifest + cache → rows, states, checks. Pure.
- `runlog.rs` — the run log: in memory for the pane, flushed line by line to
  `%LOCALAPPDATA%\PacenotesInstaller\install.log`. The UI thread owns it, as
  it owns the controls.
- `size.rs` — byte counts as a person reads them ("41.6 MB"), in binary units
  so they match Explorer.
- `worker.rs` — the one background thread. A worker NEVER touches a control;
  it posts a `Message` and the UI thread acts on it.
- `gamedir.rs` — which games exist, their dropdown labels, and the marker
  executable that identifies each one's folder, plus `detect()` which drives
  the wizard's game dropdown. Pure std, so the folder check is testable
  without a desktop. A game folder is NOT "any directory that exists"; see the
  note under Testing.
- `http/` — WinHTTP wrapper: `get(url, bearer: Option<&str>) -> Vec<u8>`
  with a progress callback. Synchronous; always called from the worker thread.
- `api.rs` — fetch manifest, download into cache (JSON isolated here).
- `cache.rs` — cache dir layout, cache.json read/write, sha verification.
- `ledger.rs` — installed.json read/write (atomic), file ownership queries.
- `engine.rs` — plan + execute the install; backup; restore; uses zip.rs.
- `sha256.rs` — small std-only sha256 (~100 lines). (No `sha2` crate.)
- `zip.rs` — zip reader (central directory + miniz_oxide inflate). The
  legacy zip WRITER for backups is no longer needed (backups are plain
  file copies) — do not write one.
- `install.rs` — dispatch to the chosen game's own configuration step, plus
  one module per game that has one (`install/rbr.rs`). There is NO shared
  abstraction over these on purpose: the games share no file layout, some
  have no plugin system, and a step may have to edit game files directly.
  The only contract is that a step reports the files it wrote, so they are
  ledgered and backed up like extracted ones.
- `i18n/` — string table lookup (localization via .rc STRINGTABLE), the
  shipped `Language` list, and the OS-UI-language default for scene 1.

## Porting map (C# legacy/DownloadManager.cs → new model)

| C# | Fate |
|---|---|
| `InitializeSupabase[/(privateKey)]` | replaced: `api.rs` + bearer access key |
| `ListAllFiles` + recursion | replaced: flat manifest fetch |
| `DownloadFile`/`saveFile` | replaced: `api.rs` download → cache (sha compare supersedes the updated_at mtime check) |
| `ExtractZipFile` | ported: `zip.rs` extract, same dir-entry handling |
| `CreateRBRConfiguration` | SUPERSEDED, measured against the live catalogue: the style packages now ship the `config\pacenotes\*.ini` files it generated, and the voice packages ship a complete `PaceNote.ini`. RBR's step EDITS `language` and `sounds` in that file and preserves the rest; it writes the file from a template only when absent. Regenerating it would discard the author's `muteReplayPacenotes`, comments and any newer key |
| `backupRBRConfiguration` | superseded by engine backup (per-file, first-touch). The C# stream bug is moot |
| `unpack/install/uninstall RBRConfiguration` | superseded by cache + ledger + engine |
| `restoreRBRBackup` (oldest-zip rule) | superseded by backup-tree restore |

Behavior users depend on and MUST survive: overlay layout lands files in the
right places; a pristine-restore path exists; config style choice produces
the same inis; missing Pacenote plugin dir → localized "install the plugin
first" error.

## Error handling

- Scaffolding phase: unwrap() tolerated.
- Final code: every user-reachable failure → `MessageBoxW` with a localized
  string, then either recover or clean exit. No silent catch-and-continue
  (the C# did this; don't copy it).
- A failed install must leave the ledger truthful: ledger updates are per-file
  batch per package, written after that package's action succeeds.

## Size audit workflow

- `tests/Test-BinarySize.ps1` — fails build over 500 kB (release gate).
- When investigating: `cargo bloat --release --target x86_64-pc-windows-msvc`.
- Budget sketch: WinHTTP+dialog skeleton ~120-180 kB; miniz ~40 kB;
  serde+serde_json ~80-120 kB; cache/ledger/engine ~40 kB. Headroom is real
  but finite — check after each milestone.

## Tests

Rust unit tests live beside the module they test (ST-007 of the
coding-standards skill): `#[cfg(test)] mod tests` at the bottom of the file,
or `<module>/tests.rs` once they outgrow it, as `src/gamedir/tests.rs` and
`src/tree/tests.rs` do. Cargo's `tests/` holds only scenario tests that drive
the public API across modules, named after the scenario, plus the PowerShell
suites. Read `tests/README.md` before adding one.

    pwsh -File tests/Run-AllTests.ps1          # everything
    pwsh -File tests/Run-AllTests.ps1 -SkipUi  # headless: logic + size gate

- `src/gamedir/tests.rs` — cross-platform `cargo test`. Game-folder identification.
- `tests/Test-VersionSync.ps1` — `[package] version` in Cargo.toml must match
  FILEVERSION/PRODUCTVERSION and the FileVersion/ProductVersion strings in
  `res/installer.rc`. The CD workflows tag releases from Cargo.toml, so drift
  would ship an .exe whose properties contradict its own tag. Bump all four
  together.
- `tests/Test-BinarySize.ps1` — the 500 kB release gate.
- `tests/Test-HttpClient.ps1` — builds and runs a real `pacenotes-server`,
  then drives `--selftest-http` against it: /health, anonymous vs
  authenticated /packages, downloads, 401, 404, and an unreachable port.
  WinHTTP cannot be unit tested, so this is the only thing that proves
  `http.rs` works. Needs no desktop, so CI runs it.
- `tests/ui/Test-UiLayout.ps1` — AUTOMATED TEST that drives the real .exe
  through user32.dll. Run it in BOTH languages (`-Language 0` / `-Language 1`)
  after ANY change to `res/installer.rc`, `ui.rs` or `i18n.rs`. The Polish pass
  is what proves the per-language STRINGTABLE lookup and the UTF-8 diacritics.
- `tests/ui/Get-UiScreenshot.ps1` — AGENT/DEVELOPER AID. Writes PNGs so a human
  or an agent can SEE the layout. Asserts nothing, so never gate on it. Use it
  whenever you change geometry: assertions cannot reveal an overlap, and a
  tab-header Z-order bug already slipped past a fully green run once.
- `tests/ui/Win32Ui.psm1` — shared helpers. Its comments record the traps
  (`GetWindowText` does not cross process boundaries; `TCM_SETCURSEL` does not
  notify the parent; `-like` eats `[Game]` prefixes).

The GUI tests need Windows with an interactive desktop session. Everything else
runs anywhere, which is the point of the lib/bin split: pure modules live in
the lib target (`src/lib.rs`) and the Windows-only bin (`src/main.rs`) owns
`http`/`ui`/`i18n`, so `cargo test` never needs Win32.

CI (`.github/workflows/ci.yml`, `installer` job) runs fmt, clippy, `cargo
test`, the version sync and the size gate — everything except the GUI tests.
The installer is excluded from the Cargo workspace, so the `server` job's
fmt/clippy do NOT reach it; that job is the only thing linting this crate.
`Run-AllTests.ps1` runs the same set plus the GUI tests, so a green local run
means a green CI run.

### Two rules for writing tests here

1. **No arbitrary inputs.** A fixture must stand for a real situation the user
   can be in. A game folder is the folder holding THAT GAME's executable, so
   the fixture is a directory containing `RichardBurnsRally_SSE.exe` — not
   `$env:TEMP`. An earlier suite used a scratch directory and passed happily
   while `validate_dir` accepted any path that existed.
2. **Assert against the source of truth, not a copy.** The UI test reads the
   expected strings out of `res/installer.rc`, so it catches a caption bound to
   the wrong id or a missing translation. Duplicating the strings into the test
   would only prove two hand-written copies agree.

If a field needs a test, that is because it requires data from a precise
context. Work out what that context is and build it; do not assert that the
field merely holds something.

### What still belongs under test, and is not yet

Pure-logic modules that are still scaffolding: `zip.rs`, `sha256.rs`,
`cache.rs` index logic, `ledger.rs`, `engine.rs` planning, `install.rs` ini
generation. Keep Win32 out of them and inject base paths/writers so they stay
testable. Engine planning (diff + exclusive groups + requires) is THE most
test-worthy logic in the installer — it gets exhaustive table tests at I7.
