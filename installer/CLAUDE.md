# installer/CLAUDE.md — Windows GUI installer

Windows-only (x86_64-pc-windows-msvc). HARD RELEASE GATE: final .exe < 500 kB.
Run `tests/Test-BinarySize.ps1` after every change that touches dependencies or adds
noticeable code. If size regresses, fix it before continuing.

## Why the binary stays small — do not break these

1. **HTTPS = WinHTTP** (`Win32_Networking_WinHttp`). The OS provides TLS
   (Schannel). Never add reqwest/hyper/rustls/native-tls/openssl.
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
- **Cache** (`cache.rs`): `%LOCALAPPDATA%\PacenotesInstaller\cache\`
  - `<sha256>.zip` — the payloads, keyed by manifest sha (integrity+dedupe)
  - `cache.json` — index: (directory, name) → {sha256, version, size,
    updated_at, category, display_name, exclusive_group, requires} — a
    snapshot of PackageInfo, so the Install view works fully OFFLINE.
  - Download = fetch → sha256 verify → move into cache. Re-download only when
    manifest sha differs from cached sha.
- **Ledger** (`ledger.rs`): per game dir,
  `Plugins\Pacenote\installer\installed.json`:
  `{ "packages": { "<directory>/<name>": { "sha256": ..., "version": ...,
     "files": ["relative\\path", ...] } } }`
  Written atomically (temp + rename) AFTER each successful package
  install/uninstall, not once at the end.
- **Backup**: before writing a file that exists on disk but is NOT owned by
  any ledger entry, copy it to `backup\<relative path>` (first time only —
  never overwrite an existing backup). Restore = copy backup tree back.
  This replaces the legacy backup/FilipekMod/unpacked staging AND the
  oldest-zip restore rule.
- **Apply** (`engine.rs`): input = user's selection (set of cached package
  refs) + ledger. Plan = uninstall (ledgered but unselected, plus any
  ledgered member of an exclusive_group superseded by a new selection) then
  install (selected but not ledgered, or sha changed = update). Enforce
  `requires` (selecting a package auto-selects/blocks on its deps being
  cached). Show the plan in the status area before running.

## Module map

- `main.rs` — entry point, common-controls init, `--selftest-http`, launches
  the wizard.
- `wizard.rs` — the UI. Layout per `DESIGN.md` rev. 2: a comctl32
  PropertySheet in wizard mode, one page per scene, with `WizardState` shared
  across pages. Scenes 1-3 (Language, Message of the day, Game folder) are
  implemented; 4-7 are I2b part 2. Read the RECORDED DEVIATIONS in
  `TASKS.md` I2b before changing the sheet: Wizard97 headers, PSM_SETNEXTTEXT
  and pszCaption all had to be worked around, and the comments say why.
- `ui.rs` — the small Win32 vocabulary the pages are written in: control
  text, combo boxes, the three modal shapes, the folder picker.
- `worker.rs` — the one background thread. A worker NEVER touches a control;
  it posts a `Message` and the UI thread acts on it.
- `gamedir.rs` — which games exist, their dropdown labels, and the marker
  executable that identifies each one's folder, plus `detect()` which drives
  the wizard's game dropdown. Pure std, so the folder check is testable
  without a desktop. A game folder is NOT "any directory that exists"; see the
  note under Testing.
- `http.rs` — WinHTTP wrapper: `get(url, bearer: Option<&str>) -> Vec<u8>`
  with a progress callback. Synchronous; always called from the worker thread.
- `api.rs` — fetch manifest, download into cache (JSON isolated here).
- `cache.rs` — cache dir layout, cache.json read/write, sha verification.
- `ledger.rs` — installed.json read/write (atomic), file ownership queries.
- `engine.rs` — plan + execute Apply; backup; restore; uses zip.rs.
- `sha256.rs` — small std-only sha256 (~100 lines). (No `sha2` crate.)
- `zip.rs` — zip reader (central directory + miniz_oxide inflate). The
  legacy zip WRITER for backups is no longer needed (backups are plain
  file copies) — do not write one.
- `install.rs` — `write_rbr_config` port (the one built-in special step)
  + RBR-specific path knowledge.
- `i18n.rs` — string table lookup (localization via .rc STRINGTABLE), the
  shipped `Language` list, and the OS-UI-language default for scene 1.

## Porting map (C# legacy/DownloadManager.cs → new model)

| C# | Fate |
|---|---|
| `InitializeSupabase[/(privateKey)]` | replaced: `api.rs` + bearer access key |
| `ListAllFiles` + recursion | replaced: flat manifest fetch |
| `DownloadFile`/`saveFile` | replaced: `api.rs` download → cache (sha compare supersedes the updated_at mtime check) |
| `ExtractZipFile` | ported: `zip.rs` extract, same dir-entry handling |
| `CreateRBRConfiguration` | PORTED as `install::write_rbr_config` — ini text byte-for-byte (modulo date); may shrink where config packages already ship inis |
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
- A failed Apply must leave the ledger truthful: ledger updates are per-file
  batch per package, written after that package's action succeeds.

## Size audit workflow

- `tests/Test-BinarySize.ps1` — fails build over 500 kB (release gate).
- When investigating: `cargo bloat --release --target x86_64-pc-windows-msvc`.
- Budget sketch: WinHTTP+dialog skeleton ~120-180 kB; miniz ~40 kB;
  serde+serde_json ~80-120 kB; cache/ledger/engine ~40 kB. Headroom is real
  but finite — check after each milestone.

## Tests — ALL of them live in `tests/`

There are no inline `#[cfg(test)]` modules in `src/`, and no validation script
anywhere else in the crate. Read `tests/README.md` before adding one.

    pwsh -File tests/Run-AllTests.ps1          # everything
    pwsh -File tests/Run-AllTests.ps1 -SkipUi  # headless: logic + size gate

- `tests/gamedir.rs` — cross-platform `cargo test`. Game-folder identification.
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
