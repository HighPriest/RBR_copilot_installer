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

## I2 — Dialog layout (mimic InstallationForm)
- [ ] Language-picker dialog (combo + OK/Cancel; Cancel exits app)
- [ ] Main dialog: package checklist (SysListView32, LVS_EX_CHECKBOXES),
      target-dir edit + Browse (SHBrowseForFolderW), token edit + "Unlock"
      button, progress bar (msctls_progress32), status static, Install /
      Uninstall / Close buttons
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

## I5 — API + integrity
- [ ] api.rs: fetch_manifest (GET /packages, parse shared::Manifest)
- [ ] sha256.rs (std-only) + unit tests against known vectors
- [ ] api::download_package → temp file, verify sha256, reject mismatch
- [ ] Skip-if-current: compare updated_at vs existing file mtime (port of
      C# saveFile early-return)

## I6 — Zip
- [ ] zip.rs reader: central directory parse, miniz_oxide inflate, extract
      with dir-entry handling matching C# ExtractZipFile
- [ ] zip.rs writer: minimal deflate/store archive for backups
- [ ] Cross-platform unit tests: round-trip write→read, extract fixture zip

## I7 — Install pipeline (port DownloadManager region by region)
- [ ] install::write_rbr_config (CreateRBRConfiguration) — ini text matches
      C# byte-for-byte modulo date; unit tests assert exact strings
- [ ] install::backup (fix noted C# stream bug; comment the deviation)
- [ ] install::unpack, install::install, install::install_package_zip
- [ ] install::uninstall, install::restore_backup (oldest-zip rule)
- [ ] Wire pipeline into Install button: backup → download → unpack →
      install → write config, with progress stages

## I8 — Auth flow
- [ ] Access-key entry → re-fetch manifest with bearer → packages from
      granted directories appear (UI groups list by directory)
- [ ] 401/403 → localized MessageBox (replaces C# TODO "throw a box")
- [ ] Optional: remember token in %APPDATA%\Pacenotes\token (plain file,
      user-scoped; document the tradeoff)

## I9 — Polish
- [ ] All unwraps in UI paths replaced with MessageBox + graceful handling
- [ ] Free-disk-space check before download (PackageInfo.size)
- [ ] Keyboard/tab order, default buttons, Esc behavior

## I10 — Release gate
- [ ] check-size.ps1 < 500 kB on release build; record final size here: ____
- [ ] cargo bloat report saved to docs/bloat.txt
- [ ] If over budget: replace serde_json with minimal manifest parser
      (planned fallback, see CLAUDE.md §5)
- [ ] Manual E2E on clean Windows VM against staging server
