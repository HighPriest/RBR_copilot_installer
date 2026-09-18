# Installer tasks (I1..I10)

Each task ends with: builds on Windows MSVC, `check-size.ps1` passes,
cross-platform `cargo test` green for logic modules.

## I1 — Toolchain + empty window
- [ ] Install target: `rustup target add x86_64-pc-windows-msvc`
- [ ] build.rs: `embed_resource::compile("res/installer.rc")`
- [ ] res/installer.rc: app manifest (common-controls v6, DPI-aware, UTF-8),
      one empty dialog template, version info
- [ ] main.rs: `wWinMain`, `InitCommonControlsEx`, show dialog, clean exit
- [ ] check-size.ps1 written and passing (baseline size recorded in TASKS)

## I2 — Dialog layout (two views)
- [ ] Language-picker dialog (combo + OK/Cancel; Cancel exits app)
- [ ] Main dialog with view toggle (tab control or two buttons):
      LIBRARY view: SysListView32 grouped by category (LVGF groups or
      category column), columns: name/version/size/state
      (cached|update available|not cached), Download button, access-key
      edit + Unlock
      INSTALL view: game-dir edit + Browse (SHBrowseForFolderW),
      SysListView32 of CACHED packages w/ checkboxes (radio behavior inside
      an exclusive_group enforced in LVN_ITEMCHANGED), Apply + Restore
      buttons, plan preview in status area
- [ ] Shared: progress bar (msctls_progress32), status static, Close
- [ ] ui.rs: control ID constants, dlgproc skeleton, enable/disable states
- [ ] i18n.rs + STRINGTABLE with en + one more language; picker switches it

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
- [ ] check-size.ps1 < 500 kB on release build; record final size here: ____
- [ ] cargo bloat report saved to docs/bloat.txt
- [ ] If over budget: replace serde_json with minimal JSON parse/write for
      Manifest + cache.json + installed.json (planned fallback, CLAUDE.md §5)
- [ ] Manual E2E on clean Windows VM: download several packages, install
      subset, switch config variant, update a package, uninstall, restore
