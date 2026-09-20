# Installer tasks (I1..I10)

Each task ends with: builds on Windows MSVC, `tests/Test-BinarySize.ps1` passes,
cross-platform `cargo test` green for logic modules.

## I1 — Toolchain + empty window
- [x] Install target: `rustup target add x86_64-pc-windows-msvc`
- [x] build.rs: `embed_resource::compile("res/installer.rc")`
- [x] res/installer.rc: app manifest (common-controls v6, DPI-aware, UTF-8),
      one empty dialog template, version info
- [x] main.rs: `InitCommonControlsEx`, show dialog, clean exit
      AMENDED: plain `fn main`, NOT `wWinMain`. `#![windows_subsystem =
      "windows"]` already yields a GUI-subsystem binary; hand-rolled wWinMain
      glue would add bytes for no behavioral gain.
- [x] Pin all dependency versions with `=` (root M0 claimed this was done, but
      the installer manifest pinned nothing)
- [x] tests/Test-BinarySize.ps1 written and passing (baseline size recorded in TASKS)

Recorded sizes (release, x86_64-pc-windows-msvc):
- do-nothing binary before I1 ......... 107,008 B (104.5 kB)
- after I1 (icon + VERSIONINFO) ....... 193,024 B (188.5 kB)

SIZE FLAG: `rbr.ico` is a single 256x256 PNG-compressed image of 85,272 B, so
the icon alone is ~83 kB = 17% of the 500 kB gate. It also ships no 16/32/48
sizes, so Windows downscales the 256 for the titlebar and taskbar (soft-looking
at small sizes). Generating a multi-resolution, recompressed .ico would both
look better and likely reclaim 40-60 kB. Not done here: it is an art asset,
not code. Revisit at I10 if the budget gets tight.

## I2 — Dialog layout (two views)

SUPERSEDED BY I2a. The Library/Install two-view shape below was built and
verified, then replaced when DESIGN.md made the app multi-game. Kept for the
record; the notes about listview columns, exclusive-group radio behavior and
the Apply plan preview describe the OLD layout. Read I2a for what ships.
- [x] Language-picker dialog (combo + OK/Cancel; Cancel exits app)
- [x] Main dialog with view toggle (tab control or two buttons):
      LIBRARY view: SysListView32 grouped by category (LVGF groups or
      category column), columns: name/version/size/state
      (cached|update available|not cached), Download button, access-key
      edit + Unlock
      INSTALL view: game-dir edit + Browse (SHBrowseForFolderW),
      SysListView32 of CACHED packages w/ checkboxes (radio behavior inside
      an exclusive_group enforced in LVN_ITEMCHANGED), Apply + Restore
      buttons, plan preview in status area
- [x] Shared: progress bar (msctls_progress32), status static, Close
- [x] ui.rs: control ID constants, dlgproc skeleton
- [x] i18n.rs + STRINGTABLE with en + one more language; picker switches it

I2 implementation notes:
- View toggle is a SysTabControl32. The tab and the two control groups are
  siblings; WM_INITDIALOG pushes the tab to HWND_BOTTOM (controls declared
  earlier in a template sit higher in Z-order, so the tab would otherwise
  paint over both views) and TCN_SELCHANGE ShowWindow()s whole groups.
- Localization is SetThreadUILanguage + per-language STRINGTABLE, so
  LoadStringW resolves the right table with no lookup code of our own.
  Polish wording reuses the legacy InstallationForm.pl.resx terms.
- Library grouping uses a Category COLUMN (the task allowed "LVGF groups or
  category column"); real LVGF groups can come later if wanted.

DEFERRED out of I2, because each needs data that does not exist until I5:
- Radio behavior inside an exclusive_group (LVN_ITEMCHANGED) — needs
  exclusive_group from the manifest.
- Apply plan preview in the status area — needs the engine (I7).
- Button enable/disable state machine — needs real cache/selection state.
Download / Unlock / Apply / Restore currently set the status line to the
localized "Not implemented yet." rather than failing silently.

Verified by driving the real .exe (scripted Win32 automation, not just a
build): both dialogs open; EnumChildWindows confirms the Library group visible
and the Install group hidden; a synthetic click on the Install tab flips them;
tab count 2; Library 5 columns, Install 4; Polish run shows "Instalator
Pacenotes" / "Biblioteka" / "Klucz dostępu:" / "Zakończ" / "Gotowy." with
diacritics intact; Browse opens the shell "Browse For Folder" dialog and Esc
dismisses it; WM_CLOSE exits the process cleanly.
Size after I2: 202,240 B (197.5 kB).

## I2a — User Interface design (DESIGN.md)

Re-lays the UI to `installer/DESIGN.md`. This REPLACES the I2 two-view
(Library/Install) shape: the app is now multi-game, so the top-level axis is
the game, not the activity.

- [x] Game ribbon across the top: one tab per supported game
      (Richard Burns Rally, Dirt Rally, BeamNG). All tabs share ONE control
      set ("identical layout and content"); switching swaps per-game STATE,
      which is far smaller than three copies of every widget.
- [x] Language widget at startup + "restart to change language" note
- [x] Game directory input + Browse button + live validation label, re-checked
      on EN_CHANGE, after Browse, and on every ribbon switch. Validated AGAINST
      THE SELECTED GAME (see the gamedir note below), not for mere existence:
      blank / not found / not this game / confirmed.
- [x] Two-column layout inside each game tab
- [x] Left column: downloaded content, one single-select list per category
      (co-driver voice, UI language, callouts style) + Install button
- [x] Install validates a pick exists in EVERY category before proceeding
- [x] Right column: Download / Backups tabs, Download is the default page
- [x] Download page: per-category lists + private access-key entry + Unlock
- [x] Backups page: backup list + Restore
- [x] Status bar beneath the columns (progress bar + status text)
- [x] Bottom: Notes tab (user guidance) + Logs tab
- [x] Errors auto-switch the bottom area to the Logs tab
- [x] Polish + English strings for all new chrome
- [x] Scripted UI tests in `tests/` (see below)

I2a implementation notes:
- Per-game state lives in an `App` struct owned by the dialog via
  GWLP_USERDATA, freed on WM_NCDESTROY. Currently it holds the game folder;
  package selections join it in I5.
- Z-ORDER TRAP: there are now three tab controls, and each
  `SetWindowPos(HWND_BOTTOM)` drops that control to the very bottom, so the
  LAST one pushed ends up lowest. The game ribbon visually contains the other
  two, so it must be pushed last — pushing it first made the ribbon's body
  paint over the Download/Backups and Notes/Logs headers, which is exactly the
  bug the screenshot aid caught after the assertions had all passed.
- Category lists are plain LISTBOXes (single-select by default), which is what
  "single item has to be selectable" needs; no LVS_EX_CHECKBOXES any more.
- GAME-FOLDER VALIDATION is per game, in `src/gamedir.rs`: a folder counts as
  a game's folder only if it holds that game's marker executable
  (`RichardBurnsRally_SSE.exe`, `dirtrally2.exe`, `BeamNG.drive.exe`) — the
  same check the legacy `InstallationForm.cs` made. The first cut accepted any
  directory that existed, which would have let the installer overlay RBR
  pacenotes into `C:\Windows`, and made the ribbon tab decorative. The status
  message names the executable so the user knows what was looked for.
  `gamedir.rs` is pure std and lives in the lib target, so the whole check is
  tested cross-platform by `tests/gamedir.rs`.

STILL DEFERRED (needs data that does not exist until I5-I7):
- The lists are empty: real content arrives with the manifest/cache in I5.
- Download / Unlock / Restore set the localized "Not implemented yet." status.
- Install enforces its preconditions but cannot yet install (engine is I7).
- Status bar shows text only; per-file name + size need the download loop (I4).

Verified via `tests/Run-AllTests.ps1`: `cargo test` green, size gate green,
`tests/ui/Test-UiLayout.ps1` 55/55 in English AND Polish, plus screenshots
reviewed for both languages across game/right/info tab combinations.
Size after I2a: 220,672 B (215.5 kB).

## Tests — consolidated under `tests/`

All installer checks live in `installer/tests/`; `src/` has no inline
`#[cfg(test)]` modules and no validation script sits elsewhere in the crate.
`check-size.ps1` moved to `tests/Test-BinarySize.ps1` (workflows and docs
updated). See `tests/README.md`.

- [x] `tests/Run-AllTests.ps1` — one entry point (`-SkipUi` for headless);
      also runs fmt + clippy, so a green local run means a green CI run
- [x] `tests/gamedir.rs` — cross-platform, 11 tests
- [x] `tests/Test-VersionSync.ps1` — Cargo.toml version vs the four
      VERSIONINFO fields in `res/installer.rc`
- [x] `tests/Test-BinarySize.ps1` — the 500 kB release gate
- [x] `tests/ui/Test-UiLayout.ps1` — AUTOMATED TEST, 55 checks per language
- [x] `tests/ui/Get-UiScreenshot.ps1` — AGENT/DEVELOPER AID, asserts nothing

Two rules these follow, and any new test must:

1. NO ARBITRARY INPUTS. A fixture stands for a real user situation. The folder
   tests build a directory containing a game's marker executable, because that
   is what the field is for. The earlier suite passed `$env:TEMP` and was green
   while the code accepted any path that existed.
2. ASSERT AGAINST THE SOURCE OF TRUTH. The UI test reads expected strings from
   `res/installer.rc`, so it catches a caption bound to the wrong id or a
   missing translation; duplicated literals would only prove two copies agree.

Consequently the "Install proceeds once all three categories have a pick" path
is NOT asserted yet — those lists are filled from the server manifest, which
arrives in I5. Injecting placeholder rows would prove only that a listbox can
hold a string. That case lands with the code that populates them.

CI runs `cargo test` + the size gate. The GUI tests need an interactive desktop
session, so they are run locally via the runner, not gated in CI.

## I3 — WinHTTP client
- [ ] http.rs: open/connect/request wrappers; `get(url, bearer) -> Result<Vec<u8>>`
- [ ] Chunked read loop with `progress: fn(read: u64, total: Option<u64>)`
- [ ] Proper error surface: WinHTTP error code → readable string
- [ ] Manual test binary flag (`--selftest-http URL`) hitting server /health

## I4 — Worker thread + progress protocol
- [ ] `WM_APP` message constants (progress %, status-string id, done, error)
- [ ] Spawn worker via std::thread; UI thread only touches controls
- [ ] Cancel support: AtomicBool checked in download loop

## I5 — API + cache
- [ ] sha256.rs (std-only) + unit tests against known vectors
- [ ] api.rs: fetch_manifest (GET /packages, parse shared::Manifest v2 incl.
      category/display_name/exclusive_group/requires)
- [ ] cache.rs: %LOCALAPPDATA%\PacenotesInstaller\cache\ layout;
      cache.json index (PackageInfo snapshot per entry); atomic writes
- [ ] api::download_to_cache → temp file → sha256 verify → move to
      cache\<sha>.zip + index update; reject mismatch
- [ ] Library state logic: not cached / cached / update available
      (manifest sha != cached sha); unit tests (pure, injected paths)

## I6 — Zip reader
- [ ] zip.rs reader: central directory parse, miniz_oxide inflate, extract
      with dir-entry handling matching C# ExtractZipFile; also
      `entries() -> Vec<relative path>` WITHOUT extracting (engine planning)
- [ ] Zip-slip guard: reject entries escaping the target dir
- [ ] Cross-platform unit tests: extract fixture zip; entries() listing
      (NO zip writer — backups are plain file copies now)

## I7 — Ledger + engine (the core)
- [ ] ledger.rs: installed.json read/write (atomic temp+rename), ownership
      query (is file owned by any package?)
- [ ] engine.rs plan(): selection + ledger + cache index →
      ordered actions [Uninstall(ref)..., Install(ref)...]; exclusive_group
      supersession; requires enforcement (block w/ message if dep not
      cached); sha-changed = reinstall
- [ ] engine.rs execute(): per package — uninstall: delete ledgered files
      (+ prune empty dirs), drop ledger entry; install: for each zip entry,
      first-touch backup if existing file unowned, extract, then write
      ledger entry
- [ ] Backup/restore: backup\<relative path> tree, never overwrite existing
      backup file; restore copies tree back over game dir
- [ ] install.rs: `write_rbr_config` port — ini text matches C# byte-for-byte
      (modulo date); unit tests assert exact strings; triggered after Apply
      when a `configs`-category package is installed
- [ ] Exhaustive engine plan() table tests + tempdir execute() tests
      (fresh install, add, remove, switch exclusive variant, update, missing
      dep, unowned-file backup, restore)

## I8 — Auth flow
- [ ] Access-key entry → re-fetch manifest with bearer → packages from
      granted directories appear in Library
- [ ] 401/403 → localized MessageBox
- [ ] Optional: remember key in %APPDATA%\PacenotesInstaller\token (plain
      file, user-scoped; document the tradeoff)

## I9 — Polish
- [ ] All unwraps in UI paths replaced with MessageBox + graceful handling
- [ ] Free-disk-space check before download (PackageInfo.size) and before
      Apply (sum of selected sizes)
- [ ] Cache housekeeping: "Remove from cache" per row; orphaned <sha>.zip
      sweep on startup
- [ ] Keyboard/tab order, default buttons, Esc behavior

## I10 — Release gate
- [ ] tests/Test-BinarySize.ps1 < 500 kB on release build; record final size here: ____
- [ ] Bump `[package] version` in Cargo.toml AND the four VERSIONINFO fields in
      res/installer.rc together; `tests/Test-VersionSync.ps1` enforces it, and
      the CD workflows tag the release from Cargo.toml
- [ ] cargo bloat report saved to docs/bloat.txt
- [ ] If over budget: replace serde_json with minimal JSON parse/write for
      Manifest + cache.json + installed.json (planned fallback, CLAUDE.md §5)
- [ ] Manual E2E on clean Windows VM: download several packages, install
      subset, switch config variant, update a package, uninstall, restore
