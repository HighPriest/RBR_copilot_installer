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

## I2a — User Interface design (DESIGN.md rev. 1)

SUPERSEDED BY I2b. DESIGN.md rev. 2 replaces the ribbon/two-column layout with
a 7-scene wizard. Reusable from I2a: `gamedir.rs` (marker-exe check), the
per-language STRINGTABLE approach, the UI test harness and its two rules.
Dropped: game ribbon, Download/Backups tabs, Notes/Logs tabs;

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
- [x] http.rs: open/connect/request wrappers; `get(url, bearer) -> Result<Vec<u8>>`
- [x] Chunked read loop (64 KiB) with `progress(read: u64, total: Option<u64>)`
- [x] Proper error surface: WinHTTP error code → readable string
- [x] Manual test binary flag (`--selftest-http URL`) hitting server /health

I3 implementation notes:
- `get_streaming()` is the real entry point and `get()` buffers on top of it.
  The task only asked for `-> Vec<u8>`, but package zips are tens of MB and
  I5 must sha256 them into a cache file as they arrive; adding the sink now
  costs a few lines and avoids rewriting the read loop later.
- Timeouts are set explicitly (10s resolve/connect, 30s send/receive). Without
  them a dead server parks the worker thread forever and Cancel does nothing,
  since cancellation is only checked between chunks.
- A response shorter than its Content-Length is an error, not a success. A
  truncated download would otherwise be written into the cache as a valid
  package and only fail later, at sha verification, with a confusing message.
- WinHTTP's error codes (12001+) are NOT in the system message table, so
  FormatMessageW is given the winhttp.dll module handle. Without that every
  network failure reads as a bare hex number.
- `--selftest-http` writes to `--out <file>` when asked and a MessageBox
  otherwise. This is a GUI-subsystem binary with no stdout; attaching to the
  parent console would mean console APIs and reopening CONOUT$ for a
  developer-only flag. `--token` was added so the authenticated paths are
  reachable too; the token is never written into the report.
- `Game::id()` / `from_id()` added to gamedir.rs, deferring to
  `pacenotes_shared::GAME_*`. The shared crate already claimed the two sides
  matched; nothing enforced it, and I5 filters each ribbon tab by this value,
  so a drift would silently empty a tab.

Verified END TO END against a real pacenotes-server, not a mock:
`tests/Test-HttpClient.ps1` 28/28 — publishes an overlay zip via
`manifest gen`, mints a credential granted one directory, then asserts
/health 200 with Content-Length honoured, anonymous /packages listing the
public package and hiding the protected one, the same listing growing once
the bearer header is sent, public and granted downloads returning exactly the
manifest's byte count, a protected download without a key giving 401 (not a
transport error), a missing package giving 404, an unreachable port giving a
WinHTTP code rendered as English prose, and the access key never appearing in
the report. Size after I3: 237,056 B (231.5 kB).

## I2b — Wizard redesign (DESIGN.md rev. 2)  ← PART 1 DONE

Replaces the I2a layout. Delivered in two parts; this is part 1 (shell +
scenes 1-3). Part 2 is scenes 4-7, which need the tree model and the engine.

Cross-crate prerequisites:
- [x] shared: `PackageInfo.tree_path: Vec<String>` (middle tree levels, e.g.
      ["polish","male"]; serde default empty). `PackageInfo.game` already
      existed from M1a.
- [x] shared: `GAME_ASSETTOCORSA`; `GAME_DIRT_RALLY` → `GAME_DIRT_RALLY2`
      with the value `dirtrally2`, per DESIGN.md rev. 2's table. The name now
      matches the value, which is Dirt Rally 2.0.
- [x] shared: CATEGORY_* renamed to DESIGN.md rev. 2's vocabulary —
      `language` / `codriver` / `style` / `pacenotes` (was `languages` /
      `sounds` / `configs`). Three sources disagreed (shared, DESIGN.md,
      this file); DESIGN.md won. Added `CHOICE_CATEGORIES`, the three the
      Configuration scene requires one pick from.
- [x] server: the `tree_path` sidecar field is read by `manifest gen`, and
      its test fixtures follow the renamed ids, so the workspace builds and
      the protocol is usable end to end.
- [ ] SERVER WORK REQUESTED — see "Server work requested" below. Not done
      here: `manifest gen` deriving a DEFAULT tree_path from the subfolders
      between the category folder and the zip, and `GET /motd`.

Wizard shell:
- [x] PropertySheet wizard host; one dialog template per scene in
      `res/installer.rc`; per-scene primary label; Back only from scene 3;
      Cancel confirms
- [x] `WizardState` (language, secret, server online, folder, detected games,
      game) owned by the sheet, replacing the I2a per-game `App`
- [x] Remove ribbon / two-column / Notes-Logs code and their strings

Scenes:
- [x] S1 Language: combo + Continue; default = OS UI language if supported
- [x] S2 MOTD: read-only text from /motd (offline fallback text); Secret edit
      (ES_PASSWORD); "Server status: Online/Offline" from /health,
      "Checking…" until answered; ACCEPT validates a non-empty secret via
      /packages when online (401 → warning modal, stay)
- [x] S3 Game folder: path edit + Browse + dropdown with "(Detecting)"
      placeholder; scan (debounced 400 ms on EN_CHANGE, immediate after
      Browse); 1 match → set + disabled, >1 → enabled with matches only,
      0 → placeholder + warning modal (after Browse and on Next only)
- [ ] S4 Download — part 2
- [ ] S5 Configuration — part 2
- [ ] S6 Installation — part 2
- [ ] S7 Summary — part 2
- [ ] Log sink: in-memory + `%LOCALAPPDATA%\PacenotesInstaller\install.log`
- [x] gamedir.rs: Dirt Rally marker `dirtrally2.exe`, Assetto Corsa added
      (`assettocorsa.exe`), BeamNG kept (DESIGN.md's table lists it; this
      file's scan list had simply omitted it); `detect(folder) -> Vec<Game>`
- [x] English + Polish strings for every caption, label and modal in S1-S3

RECORDED DEVIATIONS (DESIGN.md asked for these; here is what shipped and why):

1. CLASSIC WIZARD, NOT WIZARD97. The sheet is created with `PSH_WIZARD`. The
   Wizard97 header strip draws its title only alongside a header BITMAP;
   without one it reserved an empty band and ignored BOTH
   `PROPSHEETPAGEW::pszHeaderTitle` and `PSM_SETHEADERTITLE` — verified on
   screen. Shipping a bitmap to satisfy it would spend size budget on
   decoration. The scene title is now a static control in each page template,
   which also makes it localizable per scene. DESIGN.md allows this fallback.

2. `PSM_SETNEXTTEXT` IS IGNORED by this sheet — confirmed by sending it from
   OUTSIDE the process, where the button caption did not change either. The
   per-scene primary label is set by writing the text of comctl32's own
   button ids (`ID_WIZNEXT`/`ID_WIZFINISH` from prsht.h). Back and Cancel are
   relabelled the same way, because comctl32 supplies them in the OS language
   and a Polish wizard would otherwise carry two English buttons.

3. SCENE 1 IS NOT A SMALL WINDOW. A property sheet is one size for every page,
   sized to the largest, so scene 1 shares the wizard frame. The alternative,
   a separate modal before the sheet, would give scene 1 a different Cancel
   behavior from every other scene.

4. `PROPSHEETHEADERW::pszCaption` had no effect; the title bar is set with
   WM_SETTEXT on the sheet when each scene activates.

5. Scene 1 renders in the OS UI language, because it is shown before the user
   has chosen one. Its captions are therefore asserted to be a shipped string
   in SOME language rather than a specific one.

Part 2 (next):
- [ ] Scenes 4-7, the log sink, and the pure tree-model module
- [ ] Pure tree-model module (no Win32): manifest + cache → tree nodes with
      states; check propagation; per-category validation — cross-platform
      tests with fixtures built from the real package layout
- [ ] Uninstall/restore scene (DESIGN.md scene 3's UNINSTALL button and the
      backup-restoration window). Deferred deliberately: it restores from the
      backup tree the engine writes, which is task I7.

Tests:
- [x] tests/gamedir.rs: detect() for 0 / 1 / several markers, case-insensitive
      exe names, a directory named like a marker, AC + DR2 + BeamNG (21 tests)
- [x] tests/ui/Test-UiLayout.ps1 rewritten for the wizard: walks S1→S3 in EN
      and PL against a dead server (the deterministic offline path), asserting
      captions from installer.rc, the "(Detecting)" placeholder, the dropdown
      enabled only with several matches, Next blocked until detection, and
      Back navigation — 41 checks per language
- [x] Screenshots of all three scenes in both languages reviewed
- [ ] Modal assertions (no-marker warning, bad-secret warning) — the modal
      blocks the app thread, so driving it needs care; deferred with the
      scenes that raise more of them

Verified: `tests/Run-AllTests.ps1` green on all seven steps — fmt, clippy,
cargo test (21 gamedir + 10 shared), version sync, size gate, HTTP 28/28,
wizard 41/41 in English and 41/41 in Polish.
Size after I2b part 1: 275,456 B (269 kB) against the 500 kB gate.

## Server work requested (hand to the server agent)

Both are DESIGN.md rev. 2 requirements that the installer cannot supply for
itself. The shared protocol side is already in place.

1. `manifest gen`: derive a DEFAULT `PackageInfo.tree_path` from the folders
   between the category folder and the zip, so publishers get the Download
   scene's tree shape without hand-authoring every sidecar. The field is
   already read from the sidecar, so an explicit value must keep winning.
   Example: `packages/public/codriver/polish/male/Kamil.zip` →
   `tree_path = ["polish", "male"]`.

2. `GET /motd?lang=xx` → `text/plain` from `packages/motd/<lang>.txt`, falling
   back to `en`, 404 when there is none, no auth. Scene 2 already calls it and
   treats 404 exactly like being offline, so the installer works today and
   improves the moment this lands.

Note for whoever picks these up: `CATEGORY_*` in shared/ changed value
(`languages`→`language`, `sounds`→`codriver`, `configs`→`style`) and
`GAME_DIRT_RALLY`→`GAME_DIRT_RALLY2` (`dirtrally`→`dirtrally2`). The server's
own test fixtures were updated with it, but any sidecar .toml already authored
for real packages needs the same rename.


## I4 — Worker thread + progress protocol  (core landed with I2b part 1)

`src/worker.rs` exists because scene 2 cannot ask /health and /motd from the
UI thread. What it has now is the minimum those scenes needed; the progress
half arrives with scene 6.

- [x] `WM_APP` message constant + a `Message` enum per job, carried as a
      leaked `Box` reclaimed by the receiver
- [x] Spawn worker via std::thread; the UI thread is the only one that
      touches controls
- [x] Request ids: a scene bumps one per request so a late answer to a
      superseded request (a 10 s connect timeout landing after the user moved
      on) is recognised and dropped
- [ ] Cancel support: AtomicBool checked in download loop (`http.rs` already
      takes one; nothing cancellable runs yet)
- [ ] Messages carry what scene 6 needs: phase, current file name, bytes
      done / total; plus log lines for the shared log sink
- [ ] Scene 2 uses it for /health + /motd, scene 4 for /packages

## I5 — API + cache
- [ ] sha256.rs (std-only) + unit tests against known vectors
- [ ] api.rs: fetch_manifest (GET /packages, parse shared::Manifest incl.
      game/tree_path/category/display_name/exclusive_group/requires),
      fetch_motd, health
- [ ] cache.rs: %LOCALAPPDATA%\PacenotesInstaller\cache\ layout;
      cache.json index (PackageInfo snapshot per entry, so the tree can show
      local-only packages offline); atomic writes
- [ ] api::download_to_cache → temp file → sha256 verify → move to
      cache\<sha>.zip + index update; reject mismatch
- [ ] Cache state per leaf: not cached / downloaded / update / local only;
      unit tests (pure, injected paths)

## I6 — Zip reader
- [ ] zip.rs reader: central directory parse, miniz_oxide inflate, extract
      with dir-entry handling matching C# ExtractZipFile; also
      `entries() -> Vec<relative path>` WITHOUT extracting (needed by the
      backup phase before install)
- [ ] Zip-slip guard: reject entries escaping the target dir
- [ ] Cross-platform unit tests: extract fixture zip; entries() listing
      (NO zip writer — backups are plain file copies)

## I7 — Ledger + engine (the core)
- [ ] ledger.rs: installed.json read/write (atomic temp+rename), ownership
      query (is file owned by any package?)
- [ ] engine.rs plan(): wizard result (three picks + pacenotes packages) +
      ledger + cache index → ordered actions [Uninstall(ref)...,
      Install(ref)...]; a new pick in a category supersedes the ledgered one
      (category acts as the exclusive group for language/sounds/config);
      requires enforcement; sha-changed = reinstall
- [ ] engine.rs execute() in scene-6 phase order: download missing →
      backup (first touch, unowned files only, from zip entries()) →
      uninstall superseded → install; ledger entry written per package
- [ ] Backup/restore: backup\<relative path> tree, never overwrite an
      existing backup file; restore copies tree back (entry point per
      DESIGN.md open question 1)
- [ ] install.rs: `write_rbr_config` port — ini text matches C# byte-for-byte
      (modulo date); unit tests assert exact strings; RBR only, runs after
      the install phase using the Pacenotes Style + Language picks
- [ ] Exhaustive plan() table tests + tempdir execute() tests (fresh
      install, switch style, switch voice, update, local-only package,
      missing dep, unowned-file backup, restore)

## I8 — Auth flow
- [ ] Secret from scene 2 used as bearer for /packages and /download
- [ ] 401 on ACCEPT → warning modal; 401/403 during download → logged
      error, failure summary
- [ ] Optional: remember the secret in %APPDATA%\PacenotesInstaller\token
      (plain file, user-scoped; document the tradeoff) and prefill scene 2

## I9 — Polish
- [ ] All unwraps in UI paths replaced with modal/log + graceful handling
- [ ] Free-disk-space check before scene 6 (sizes of missing downloads +
      installs)
- [ ] Cache housekeeping: orphaned <sha>.zip sweep on startup; decide where
      "remove from cache" lives now that there is no library view
- [ ] Remember last game folder per game; prefill scene 3
- [ ] Keyboard/tab order, default buttons, Esc = Cancel behavior per scene

## I10 — Release gate
- [ ] tests/Test-BinarySize.ps1 < 500 kB on release build; record final size here: ____
- [ ] Bump `[package] version` in Cargo.toml AND the four VERSIONINFO fields in
      res/installer.rc together; `tests/Test-VersionSync.ps1` enforces it, and
      the CD workflows tag the release from Cargo.toml
- [ ] cargo bloat report saved to docs/bloat.txt
- [ ] If over budget: replace serde_json with minimal JSON parse/write for
      Manifest + cache.json + installed.json (planned fallback, CLAUDE.md §5);
      revisit the 83 kB icon (see I1 size flag)
- [ ] Manual E2E on clean Windows VM, per game: full wizard online with and
      without secret, offline run from cache, switch style on re-run,
      failure path (server dropped mid-download) lands on failure summary
