# Installer tasks (I1..I10)

Each task ends with: builds on Windows MSVC, `tests/Test-BinarySize.ps1` passes,
cross-platform `cargo test` green for logic modules.

## I1 — Toolchain + window  (done)

- [x] `rustup target add x86_64-pc-windows-msvc`
- [x] build.rs compiles `res/installer.rc` via `embed_resource`
- [x] res/installer.rc: app manifest (common-controls v6, DPI-aware, UTF-8),
      icon, VERSIONINFO
- [x] Plain `fn main`, not `wWinMain`: `#![windows_subsystem = "windows"]`
      already yields a GUI-subsystem binary, so the glue would cost bytes for
      no behavioral gain
- [x] Every dependency pinned with `=`
- [x] `tests/Test-BinarySize.ps1`, the 500 kB release gate

SIZE FLAG for I10: `rbr.ico` is a single 256x256 PNG-compressed image of
85,272 B, so the icon alone is ~83 kB — 17% of the gate. It also ships no
16/32/48 sizes, so Windows downscales the 256 for the titlebar and taskbar.
A multi-resolution, recompressed .ico would look better AND likely reclaim
40-60 kB. Not done: it is an art asset, not code.

## I2 / I2a — pre-wizard layouts  (superseded by I2b)

Two layouts were built and verified before `DESIGN.md` settled on the wizard:
a Library/Install two-view dialog, then a game-ribbon with two columns and
Notes/Logs tabs. Both are gone from the code. What survived them lives on in
the current design rather than here — the marker-executable folder check
(`gamedir.rs`), the per-language STRINGTABLE approach, and the UI test
harness with its two rules. See the CHANGELOG and git history for the detail.

## Tests — consolidated under `tests/`

All installer checks live in `installer/tests/`; `src/` has no inline
`#[cfg(test)]` modules and no validation script sits elsewhere in the crate.
The two rules every test here follows, and why, are in `tests/README.md` —
read it before adding one.

- [x] `tests/Run-AllTests.ps1` — one entry point (`-SkipUi` for headless);
      also runs fmt + clippy, so a green local run means a green CI run
- [x] `tests/gamedir.rs` — cross-platform game-folder identification
- [x] `tests/Test-VersionSync.ps1` — Cargo.toml version vs the four
      VERSIONINFO fields in `res/installer.rc`
- [x] `tests/Test-BinarySize.ps1` — the 500 kB release gate
- [x] `tests/Test-HttpClient.ps1` — the WinHTTP client against a live server
- [x] `tests/ui/Test-UiLayout.ps1` — walks the wizard, per language
- [x] `tests/ui/Test-WizardDownload.ps1` — scenes 1-4 against a LIVE server
      holding a nested catalogue
- [x] `tests/ui/Get-UiScreenshot.ps1` — AGENT/DEVELOPER AID, asserts nothing

CI runs everything except the GUI tests, which need an interactive desktop
session and are run locally through the runner.

## I3 — WinHTTP client  (done)

- [x] `http.rs`: open/connect/request wrappers; `get(url, bearer)` and
      `get_streaming(...)` with a sink, because package zips are tens of MB
      and I5 sha256s them into a cache file as they arrive
- [x] Chunked read loop (64 KiB) with `progress(read, total)`
- [x] Explicit timeouts (10s resolve/connect, 30s send/receive) — without
      them a dead server parks the worker thread and Cancel does nothing,
      since cancellation is only checked between chunks
- [x] A body shorter than its Content-Length is an error, not a success
- [x] WinHTTP error code → readable string (FormatMessageW must be given the
      winhttp.dll module; those codes are not in the system message table)
- [x] `--selftest-http URL [--token T] [--out FILE]`; writes a report to the
      file when asked and a MessageBox otherwise, since a GUI-subsystem
      binary has no stdout. The token is never written into the report.

Verified end to end against a real `pacenotes-server`, not a mock —
`tests/Test-HttpClient.ps1`.

## I2b — Wizard  (part 1 done: shell + scenes 1-3)

Delivered in two parts. Part 2 is scenes 4-7, which need the tree model and
the install engine.

Protocol groundwork (shared), done:
- [x] The tree comes from the package's STORAGE PATH: `PackageInfo.name` is
      the path under its access directory (`codriver/polish/male/Kamil`), and
      `category()` / `tree_levels()` / `leaf()` derive the tree from it
- [x] `valid_name` — `name` is a lookup key that LOOKS like a path, so this
      is the choke point that stops it becoming one. `PackageRef::parse`
      splits on the FIRST separator, so a ref can point at a nested package
- [x] `GAME_ASSETTOCORSA`, `GAME_DIRT_RALLY2` (`dirtrally2`)
- [x] Category vocabulary `language` / `codriver` / `style` / `pacenotes`,
      plus `CHOICE_CATEGORIES` — the three scene 5 requires a pick from

Wizard shell, done:
- [x] PropertySheet host, one dialog template per scene, per-scene primary
      label, Back only from scene 3, Cancel confirms
- [x] `WizardState` (language, secret, server online, folder, detected games,
      game) owned by the sheet
- [x] Old ribbon / two-column / Notes-Logs code and strings removed

Scenes:
- [x] S1 Language: combo + Continue; default = OS UI language if supported
- [x] S2 MOTD: text from /motd with an offline fallback; Secret edit;
      "Server status" from /health; ACCEPT validates a non-empty secret via
      /packages when online (401 → warning modal, stay)
- [x] S3 Game folder: path edit + Browse + dropdown with "(Detecting)";
      scan debounced 400 ms on EN_CHANGE, immediate after Browse; 1 match →
      set + disabled, >1 → enabled with matches only, 0 → placeholder +
      warning modal (after Browse and on Next only)
- [x] `gamedir.rs`: `detect(folder) -> Vec<Game>`, all four games
- [x] English + Polish strings for every caption, label and modal in S1-S3
- [x] S4 Download: SysTreeView32 + TVS_CHECKBOXES + partial checkboxes, built
      from `/packages?game=<id>`; parent/child check propagation; Download
      refuses until every category present has a pick and the modal names the
      ones missing. Offline or a failed fetch falls back to the cache rather
      than erroring.
- [x] Pure tree model (`src/tree.rs`, no Win32): manifest + cache → nodes
      with states, check propagation, per-category validation. Cross-platform
      tests with fixtures shaped like the real layout (`style/Descriptive`,
      `codriver/polish/male/Kamil`)
- [ ] S5 Configuration — three lists from the leaves scene 4 committed
- [ ] S6 Installation — needs the engine (I7)
- [ ] S7 Summary
- [ ] Restore scene (scene 3's UNINSTALL) — needs the backup tree the engine
      writes, so it follows I7
- [ ] Log sink: in-memory + `%LOCALAPPDATA%\PacenotesInstaller\install.log`

How the wizard host actually behaves, and the three OS behaviours worked
around to get there, are in `DESIGN.md` under Implementation notes. Read that
before touching the sheet.

Tests:
- [x] `tests/gamedir.rs`: detect() for 0 / 1 / several markers,
      case-insensitive exe names, a directory named like a marker (21 tests)
- [x] `tests/ui/Test-UiLayout.ps1` walks S1→S3 in EN and PL against a dead
      server — the deterministic offline path — asserting captions read from
      installer.rc, the "(Detecting)" placeholder, the dropdown enabled only
      with several matches, Next blocked until detection, and Back navigation
- [x] Screenshots of all three scenes in both languages reviewed
- [ ] Modal assertions (no-marker warning, bad-secret warning): the modal
      blocks the app thread, so driving it needs care. Deferred to land with
      the scenes that raise more of them.

SIZE FLAG — serde_json has landed. Scene 4 is the first code to parse JSON,
so `serde_json` finally linked and the binary jumped 269 → 329.5 kB, 66% of
the gate. Still to come: miniz_oxide at I6 (~40 kB), plus cache/ledger/engine.
That projects to roughly 410 kB, which fits — but the two levers in
installer/CLAUDE.md §5 are now worth remembering rather than theoretical: the
83 kB icon (see I1) and swapping serde_json for a hand-rolled parser over the
three documents the installer actually reads.

Recorded sizes (release, x86_64-pc-windows-msvc), against the 500 kB gate:

| After | Bytes | |
| --- | --- | --- |
| nothing (baseline) | 107,008 | 104.5 kB |
| I1 — icon + VERSIONINFO | 193,024 | 188.5 kB |
| I3 — WinHTTP client | 237,056 | 231.5 kB |
| I2b part 1 — wizard + worker | 275,456 | 269 kB |
| I2b part 2 — tree model + scene 4 | 337408 | 329.5 kB |
| I5 — sha256 + cache + verified download | 402432 | 393 kB |

## Protocol as it stands

The server serves every game from one instance. Three consequences the
installer must honour, and which the remaining tasks below carry:

1. Download URLs lead with the game:
   `GET /download/{game}/{directory}/{*name}`. Build the tail with
   `PackageInfo::download_path()` — never assemble it by hand.
2. `GET /packages?game=<id>` filters server-side. Scene 4 asks for the
   detected game rather than filtering a whole-catalogue document.
3. `"directory/name"` is NOT unique across games, so the cache index and the
   install ledger must key on the game too.

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
- [x] Scene 2 uses it for /health + /motd
- [ ] Scene 4 uses it for `/packages?game=<id>` — the game is known by then,
      so ask the server for one game's manifest

## I5 — API + cache
- [x] sha256.rs (std-only), tested against the published FIPS/CAVP vectors
      AND against chunked input — the chunking case caught a real buffering
      bug that every published vector missed, because they arrive in one call
- [x] api.rs: `fetch_manifest(game)` → `GET /packages?game=<id>`, so the
      server filters to one game rather than the client sifting a
      whole-catalogue document; parse `shared::Manifest` incl.
      game/name-as-path/display_name/exclusive_group/requires
- [x] api.rs: fetch_motd, health
- [x] cache.rs: %LOCALAPPDATA%\PacenotesInstaller\cache\ layout;
      cache.json index (PackageInfo snapshot per entry, so the tree can show
      local-only packages offline); atomic writes
- [x] cache.json keys include the game. `"directory/name"` is not unique
      across games now that one server serves all of them, so a bare ref
      would let one game's package shadow another's — use
      `PackageInfo::download_path()` (`game/directory/name`) or an explicit
      (game, directory, name) triple. Test two games holding the same
      directory+name.
- [x] api::download_to_cache: builds the URL with
      `PackageInfo::download_path()`, never by hand — the route leads with
      the game (`/download/{game}/{directory}/{*name}`) and a name may need
      percent-encoding (e.g. `style/Numeric Standard`). Then temp file →
      sha256 verify → move to cache\<sha>.zip + index update; reject mismatch
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
- [ ] Ledger keys MUST include the game, for the same reason as the cache
      index: `"directory/name"` no longer identifies a package on its own.
      A ledger is per game FOLDER, so in practice one game — but two games
      installed from one cache must not collide, and a key that silently
      means "whichever game wrote it last" is a bug waiting for the first
      user with both installed.
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
