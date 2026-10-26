# Installer tasks (I1..I10)

Each task ends with: builds on Windows MSVC, `tests/Test-BinarySize.ps1` passes,
cross-platform `cargo test` green for logic modules.

## Where this stands (2026-10-26)

I1-I8 are done. The wizard runs end to end on both of scene 3's routes: it
downloads and verifies into the cache, installs into the game folder, backs
up what it replaces, records what it wrote, runs RBR's configuration step,
takes a restore point and reports conflicts to the server.

**The one functional gap is the restore SCENE.** Points have been written
on every run since I7 and nothing can read them back, so users accumulate
a safety net they cannot use, and scene 3's Restore/Uninstall button
answers "not implemented yet". See "Restore" below.

After that it is I9 (polish), I10 (release gate) and the small pre-release
cluster the review lists under Period 2.

**Size:** 584.5 kB against the 600 kB gate. 15.5 kB of headroom, and
`rbr.ico` is 83 kB of the binary — the first place to look if the restore
scene does not fit.

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
- [x] `tests/tree.rs`, `tests/choices.rs` — the two scene models
- [x] `tests/sha256.rs`, `tests/cache.rs` — the digest and the package cache
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
      warning modal (after Browse and on a route button only)
- [x] S3's **three route buttons** (2026-10-26): Download/Update → scene 4,
      Configure/Install → straight to scene 5, Restore/Uninstall → the
      restore scene (not built; answers "not implemented yet", and ships
      early so the page's shape does not change under users later).
      In-page buttons with the sheet's Next left disabled, because they are
      alternatives rather than one primary and two afterthoughts, and each
      route is then one click.
      - Two columns. Download/Update is the MAIN flow and the shape says
        so: default button, widest, and as tall as the other two together,
        alone on the right. The secondary pair stacks in a narrower column
        on the left, its top and bottom exactly the big button's, so the
        three read as one block. `Test-UiLayout.ps1` measures that
        arrangement — the column, the stacking order, the relative width
        and the matching height — because "looks primary" is the whole
        point of it and nothing else would catch a regression
      `Route` in `wizard/state.rs` decides only what is SKIPPED — both routes end
      in the same questions and the same install, reading the same tree, so
      there is no separate "offline" path to keep in step. The jumps use
      `PSM_SETCURSELID`, and scene 5's Back returns to whichever page the
      user actually came from
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
- [x] S4b Downloading (2026-10-26) — the download phase on a page of its
      own, between the tree and the questions. Same template and the same
      handlers as scene 6 (`init_progress`, `on_run_progress`,
      `on_run_cancel`, shared control ids), because it is the same screen at
      an earlier moment. Starts itself, verifies each payload, walks on to
      scene 5; a failure or a cancel ends the run at the summary instead,
      since there is nothing to configure that could honestly be installed
- [x] S5 Configuration — three single-select lists built from the leaves
      scene 4 committed, each option labelled with its path inside its
      category (`polish / male / Kamil`), a sole option preselected, Install
      refusing until every list that HAS options has a pick. Pure model in
      `src/choices.rs`; the picks survive a trip Back to scene 4 and forward
      again, as long as the package is still ticked
- [x] S5 defaults to WHAT IS ALREADY INSTALLED (2026-10-26). Found by the
      UI suite the moment the Configure/Install route existed: that route is
      for changing an install, so blank questions would make the user
      re-answer last time's choices — and a selection that omitted an
      installed package would uninstall it. A pick of the user's own still
      wins over the default
- [x] One page size for every scene is now 330x215 DLU, forced by scene 5:
      three lists have to be visible at once, and full width because a
      co-driver is identified by its whole path — a third of the width clips
      exactly the part that tells two voices apart. Scene 4's tree and scene
      2's MOTD took the extra room too; scene 1 has space around it, which
      DESIGN.md allows
- [x] S6 Installation — phase label, progress bar, current package with its
      own byte count, live log pane. The DOWNLOAD phase is real and verified;
      backup and install wait for I6/I7 and say so in the log rather than
      being faked. No Back and no Next: the page runs itself and walks on to
      the summary. Cancel is enabled only while the download can still be
      stopped, and it VETOES the close — the flag is raised, the worker
      reports `Cancelled` between chunks and the wizard reaches scene 7,
      instead of the window vanishing while a thread still holds its handle
- [x] S7 Summary — result, game and folder, the three picks, the no-choice
      packages, how much came down the wire, and where the log file is. The
      log pane is hidden on success with the detail block stretched over it,
      and shown on failure. Finish closes; Cancel is disabled, since there is
      nothing left to stop
- [x] The result headline reported a download with no install while that
      was the truth; since I7 it is `IDS_S7_OK` and reports what changed
- [ ] Restore scene (scene 3's Restore/Uninstall) — UNBLOCKED since I7, and
      now the last functional gap: the button exists and answers "not
      implemented yet", while every run writes a restore point that nothing
      can read back. Specified under "Restore" below
- [x] Log sink (`src/runlog.rs`): in-memory + a line-by-line flush to
      `<game dir>\.pacenotes\logs\install-<stamp>.log`, one file per run,
      newest five kept. Flushed per line because the run worth reading
      about is the one that died; buffering would drop exactly the
      interesting part
      - Moved out of `%LOCALAPPDATA%` on 2026-10-26: the log belongs with
        the install it describes, beside the ledger, the backups and the
        restore points, and the installer should not scatter state through
        a user's profile. The cache stays there because it is shared by
        every game folder
      - Dated rather than truncated, because re-opening the installer to
        read what went wrong used to erase the run being read
        (DESIGN-REVIEW §7 asked for at least three)
      - Attached when scene 3 settles a folder, replaying the lines from
        before that, so the file is the whole run

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
- [x] `tests/choices.rs`: scene 5's model against the real catalogue's shape —
      list order, the path label that tells two same-named voices apart,
      preselection, `pacenotes` as a no-choice extra, and a pick surviving
      (or correctly not surviving) a change of ticks on scene 4 (9 tests)
- [x] `tests/ui/Test-WizardDownload.ps1` now walks on into scene 5: ticking
      every category gets past the Download gate, each list offers exactly
      what the catalogue filed under it, nothing is preselected where there
      is a real choice, Install warns and names the unanswered lists, and
      three picks satisfy it
- [x] Screenshots of all five scenes in both languages reviewed, scenes 4-5
      against the live public catalogue (`-Server`), which is the only place
      the longest real labels show up
- [x] Modal assertions (missing-category warning, missing-pick warning): both
      gates are asserted, including that the modal NAMES what is missing.
      This is also what exposed `Dismiss-Modal` never closing a box — see
      trap 7 in `tests/README.md`.

SIZE FLAG — serde_json has landed. Scene 4 is the first code to parse JSON,
so `serde_json` finally linked and the binary jumped 269 → 329.5 kB, 66% of
the gate. Still to come: miniz_oxide at I6 (~40 kB), plus cache/ledger/engine.
That projects to roughly 410 kB, which fits — but the two levers in
installer/CLAUDE.md §5 are now worth remembering rather than theoretical: the
83 kB icon (see I1) and swapping serde_json for a hand-rolled parser over the
three documents the installer actually reads.

Recorded sizes (release, x86_64-pc-windows-msvc). The gate was 500 kB
through I5 and I2b, and 600 kB from I7 on — see below.

| After | Bytes | |
| --- | --- | --- |
| nothing (baseline) | 107,008 | 104.5 kB |
| I1 — icon + VERSIONINFO | 193,024 | 188.5 kB |
| I3 — WinHTTP client | 237,056 | 231.5 kB |
| I2b part 1 — wizard + worker | 275,456 | 269 kB |
| I2b part 2 — tree model + scene 4 | 337408 | 329.5 kB |
| I5 — sha256 + cache + verified download | 402432 | 393 kB |
| I2b — scene 5 + the bigger page | 412160 | 402.5 kB |
| I2b — scenes 6 and 7 + the run log | 427520 | 417.5 kB |
| I7b — the ledger reachable from main | 452608 | 442 kB |
| I7f — the engine wired in (GATE RAISED to 600 kB) | 553984 | 541 kB |

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

## I4 — Worker thread + progress protocol  (done)

`src/worker.rs` exists because scene 2 cannot ask /health and /motd from the
UI thread. Scene 6 completed it: a job that reports as it goes, and one that
can be stopped.

- [x] `WM_APP` message constant + a `Message` enum per job, carried as a
      leaked `Box` reclaimed by the receiver
- [x] Spawn worker via std::thread; the UI thread is the only one that
      touches controls
- [x] Request ids: a scene bumps one per request so a late answer to a
      superseded request (a 10 s connect timeout landing after the user moved
      on) is recognised and dropped
- [x] Cancel support: an `Arc<AtomicBool>` the UI raises and the download
      loop reads between chunks. Scene 6 vetoes the window close and waits
      for the worker to report `Cancelled`, so no thread is left posting to
      a page that has gone
- [x] `WM_WORKER_PROGRESS` carries what scene 6 needs: phase, the package
      being worked on, its own bytes done/total, the phase's bytes
      done/total, and a log line when there is one. Posts are throttled to
      10 a second — a 64 KiB chunk of a 40 MB package arrives hundreds of
      times a second, and a post per chunk would spend the UI thread
      redrawing a bar
- [x] `spawn_reporting` hands the job a `Reporter`, not the window handle,
      so the "a worker never touches a control" rule holds by construction
- [x] Scene 2 uses it for /health + /motd
- [x] Scene 4 uses it for `/packages?game=<id>`, scene 6 for the downloads

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
- [x] Cache state per leaf: not cached / downloaded / update / local only
      (`tree::LeafState`), with scene 4 reading the real index — an entry
      whose payload has vanished is withheld rather than offered and then
      failing at install time. Pure tests with injected paths in
      `tests/tree.rs` and `tests/cache.rs`

## I6 — Zip reader  (done)
- [x] zip.rs reader: EOCD scan, central-directory walk, miniz_oxide inflate
      (method 8) and stored (method 0), extract creating the directories a
      .NET zip does NOT carry entries for — which is every real package,
      since `Plugins/` and the directory holding a nested file are never
      entries. Overwrites, as C# ExtractZipFile did: a package is an overlay
      and owns its files
- [x] `entries() -> Vec<String>` WITHOUT extracting, for the backup phase.
      Directory entries are excluded: an empty directory is not something to
      back up, ledger or delete
- [x] Paths come back `/`-separated on every host, because they are the
      ledger's keys and a ledger must read the same on another machine. The
      filesystem separator appears only when joining onto a real directory
- [x] Zip-slip guard (`safe_relative`): rejects `..` segments, a leading
      separator, and anything containing `:` (drive, drive-relative path, or
      an NTFS alternate data stream). `\` counts as a separator, because
      Windows archivers write it and treating it as an ordinary character
      would both mis-name the file and let `..\..\x` through. A package with
      such an entry fails WHOLE — listing it fails too, so nothing is
      half-installed
- [x] CRC32 per entry. The sha256 was verified once, at download, possibly
      months before the install; the cache file has been on a disk since.
      The checksum is already in the archive, and checking it is what stops
      rotted bytes reaching a game folder. Nibble table (64 B) rather than
      the usual 256-entry one (1 kB), for the budget
- [x] Zip64 is detected and refused by name, rather than misread
- [x] CP437 for entry names without the UTF-8 flag, as .NET does — the
      encoding the existing packages were written with
- [x] Cross-platform tests (11) against REAL archives: `Compress-Archive`
      output, deflated and stored, with the zip-slip case made by renaming
      an entry in place (`aa/evil.txt` is as long as `../evil.txt`, so no
      offset moves and no zip writer is needed). See
      `tests/fixtures/README.md`
- [x] MANUAL CHECK `cargo test --test zip -- --ignored` over a directory of
      real packages. Run against the live catalogue: 24 packages, 11,872
      files, every one listed, extracted and CRC-verified

SIZE NOTE: still 417.5 kB — unchanged, because `main.rs` does not call the
reader yet, so LTO drops it and miniz_oxide with it. The ~40 kB this costs
lands with I7, when the engine actually extracts something. Do not read the
unchanged number as "zip was free".

## I7 — Ledger + engine (the core)

DECIDED before starting. The first three came from reading what the live
catalogue actually ships; the rest answer `DESIGN-REVIEW.md` Period 1.
Reasoning lives in `installer/CLAUDE.md` and `installer/DESIGN.md`.

1. Installer state lives in ONE place per game folder, the same for every
   game: `.pacenotes\`.
2. Each game gets its OWN configuration step. No shared abstraction.
3. A file is backed up only when its bytes DIFFER from what replaces it.
4. The ledger records per-file `sha256` and `prior`, case-folded keys with
   the original spelling, and a write-ahead `state` (review P2-P6).
5. Overlapping files: last writer wins, logged AND reported to the server
   (P7). No gate — a user is never blocked by a catalogue mistake they
   cannot fix.
6. `exclusive_group` is enforced in the engine only (P8).
7. Choice categories are per game, from the COMPILED-IN schema
   (`schemas/<game>.toml` → `schema.rs`), with localized labels and
   explanations in the `.rc` tables; only they are mandatory on scene 4
   (P9). This is why the installer checks the server's schema hash (S11) —
   the "minimum manifest version" this list first called for.
8. Restore works on dated restore points, and REMOVES what was installed
   after the chosen one (P14).

### Sequencing (decided when I7 started)

I7 is too big for one commit, and the pieces have a natural order — each
leaves the repo buildable and tested:

- **I7a** (done) `zip.rs` hardening: the P5 name rules, duplicate entries,
  and entries carrying their SIZE (the free-space pre-flight needs it).
- **I7b** (done) `ledger.rs` — pure data + atomic IO + ownership queries.
- **I7c** (done) `engine.rs plan()` — a pure function over selection + ledger +
  cache. Table tests, no filesystem.
- **I7d** (done) `engine.rs execute()`, restore points, backup. Tempdir tests.
- **I7e** (done) `install.rs` — the RBR configuration step.
- **I7f** (done) Wiring: scene 6's new phases, the ledger merged into the
  tree (P1), the removal confirmation, conflict detection and logging.

NOT in I7, though Period 1 decided them:

- **The restore SCENE** (P14) needs the points to exist first, and the
  review already places the scene in period 2. I7 WRITES restore points
  from the first install — early runs must not be the ones with no way
  back — and the UI to use them follows.
- **Per-game choice categories (P9) and the install-schema check (S11)**
  were a separate task, done 2026-10-25. They touch `shared`, `tree.rs`,
  `choices.rs`, `wizard.rs` and both string tables, and the engine does not
  care which categories are "choice" ones: it is handed a selection. Mixing
  that refactor into the engine would have made both impossible to review.
- **The conflict REPORT** (`POST /report/conflict`) waited for the server
  endpoint, `server/TASKS.md` S10, and landed with it on 2026-10-25. I7
  detected overlaps and logged them; posting them was a few lines once
  there was something to post to.

Two corrections to what Period 1 recorded, found while planning:

- **`prior: "identical"` cannot rely on pinning.** The cache is per-user,
  the ledger lives in the game folder, and the sweep cannot know which
  folders reference which payloads without a registry of game folders.
  So pinning is best-effort, and the CORRECTNESS rule is: if the original
  cannot be recovered (payload gone, entry missing), leave the file alone
  and log it. Never delete what cannot be put back.
- **Pre-flight facts that need Win32 are injected**, not called:
  `engine.rs` has to stay testable off Windows, so free disk space and
  "is the game running" arrive as values the caller measured.

### Ledger (`ledger.rs`)

- [x] `.pacenotes\installed.json` read/write, atomic temp+rename, with the
      entry shape in `installer/CLAUDE.md`
- [x] Keys carry the GAME: `"directory/name"` stopped identifying a package
      when one server began serving every game. `ledger.rs` stores whatever
      key it is handed; every caller hands it `PackageInfo::download_path()`
- [x] Per-file: `path` as written, case-folded map key, `sha256` of what we
      wrote, and `prior` (`absent` | `backup { point }` | `identical`).
      `backup` carries the restore point holding the original — a file
      replaced in several runs has a copy in several points, and only the
      one taken when THIS package overwrote it is the right one
- [x] `exclusive_group` recorded AS INSTALLED, so uninstall decisions work
      offline with no manifest to consult
- [x] `owners_of(path)` over case-folded keys. A ledgered file is deleted
      only when no other entry still lists it
- [x] `state: "installing"` written BEFORE extracting. Without it,
      half-written files from a failed run are in no entry, so the next run
      backs them up AS THE PRISTINE ORIGINALS and a later restore puts the
      broken ones back
- [x] Recovery happens in `plan()`, NOT in a separate scan at startup: an
      `installing` entry is uninstalled and — if still selected — laid
      again, because its files may be partial and the record cannot be
      trusted. Every run that reaches planning therefore cleans up after
      the last one. `Ledger::unfinished()` exists for diagnostics and the
      restore scene; nothing calls it yet
- [x] Beyond the plan: an unreadable ledger is never overwritten. It may
      be a newer build's, and flattening it would strand every package it
      records as files nothing owns and nothing can remove

### Engine (`engine.rs`)

- [x] plan(): picks + `pacenotes` extras + ledger + cache → ordered actions.
      A new pick supersedes the ledgered member of its category or
      `exclusive_group`; sha changed = reinstall; `requires` enforced
      within the game
- [x] `requires` is satisfied by the SELECTION and nothing else. "Already
      installed" does not count, because anything installed and unselected
      is uninstalled in the same run — accepting it would approve a plan
      that removes the dependency and keeps what needs it
- [x] Uninstall before install, always: the three style packages ship
      identical path lists, so the other order deletes what was just written
- [x] Pre-flight BEFORE touching anything: free space for installs plus
      the copies taken of what they replace, and a writability probe
- [x] Each payload is verified by its SHA256, not by CRC-checking every
      entry as this list first said. The hash covers every byte of the
      archive rather than each member separately, so it is the stronger
      check, and it reads the file instead of decompressing it — perhaps
      twenty times cheaper. Per-entry CRCs are still checked during
      extraction, by `zip.rs`
- [ ] "The game is not running" — the engine refuses when told
      (`HostFacts::game_running`), but nothing measures it yet:
      scene 6 (`wizard/scene/installation.rs`) passes `false` with a TODO.
      Needs the marker process check, which is I9's. Until then a run
      against an open game fails partway, which is what the write-ahead
      state is for
- [x] Each file written to a temp name then renamed, so a crash never
      leaves a truncated file in the game folder
- [x] Uninstall by `prior`: `backup` → put the restore-point copy back;
      `identical` → re-extract that one file from the cached payload;
      `absent` → delete. And only when the on-disk sha still matches what
      we wrote: Steam's verify, the RSF installer or the user's own editor
      may have replaced it, and someone else's work is not ours to remove
- [x] An `identical` entry can only be honoured while its payload is in
      the cache. Pinning it against the I9 sweep is BEST EFFORT — the cache
      is per-user and the ledger lives in the game folder, so the sweep
      cannot see what references what. The correctness rule is therefore:
      when the original cannot be recovered, leave the file alone and log
      it. Never delete what cannot be put back
- [x] Overlaps: `overlaps()` is a separate pure function over the file
      lists the pre-flight already read, called by `worker::apply` between
      planning and executing. Install order decides, the later package
      wins, and the log names them all. Nothing is blocked
- [x] Known and accepted: after uninstalling the winner of an overlap the
      file survives, holding the winner's bytes, because another entry
      still owns it. Logged at the moment it happens — it is the only way
      anyone finds out
- [x] Emptied directories are pruned on uninstall, deepest first, so a
      removed mod does not leave a tree that still looks installed

### Conflict reporting (needs the server endpoint)

- [x] Detected and logged, naming every package that writes the shared
      file and in which order
- [x] `POST /report/conflict` fire-and-forget after an install that saw an
      overlap. The endpoint EXISTS now (2026-10-25); the body is
      `shared::ConflictReport` — `{game, installer, conflicts: [{file,
      packages}]}` — one entry per shared FILE, with every package that
      wrote it as a "directory/name" ref (scoped by `game`) in install
      order, the last being the winner. No timestamp: the server stamps
      its own. Never blocks the install, never fails the run, times out
      fast; 202 = stored, 429 = back off (`Retry-After`), anything else =
      log and move on. `api::report_conflicts`, called by `worker::apply`
      AFTER `execute()` so what is reported is what actually happened
- [x] Trimmed to fit the server's 8 KiB cap, measured rather than guessed
      (`api::fitting_conflicts`): it is an unauthenticated endpoint, so a
      report over the cap is answered 413 and the operator learns nothing
- [x] `Overlap::refs()` turns the engine's `game/directory/name` keys into
      the "directory/name" refs the report carries — the report names its
      game once. A key that will not split is DROPPED, not guessed at: the
      server discards bad entries silently, so one malformed ref would
      cost the whole report its meaning
- [x] It carries no user identity and no local paths — package keys and
      relative paths are catalogue data the server already has. Posted
      with NO bearer token even when the run has one, and the log says the
      report was sent, so a user can see what left their machine
- [x] `http.rs` grew `post_json`; `get_streaming` and it now share one
      request path, which is a verb and an optional body apart
- [x] Server side is `server/TASKS.md` S10 (done 2026-10-25)

### Per-game configuration step (`install.rs`)

- [x] `configure(game, game_dir, picks)` dispatching on `Game`, returning
      the files written/changed for the ledger plus any notes for the log.
      `picks` is a category → package-key map, NOT the whole
      `Configuration`: a game takes only the categories it understands,
      which is what keeps `install.rs` ignorant of any one game's question
      list
- [x] RBR's step lives in `install/rbr.rs`, beside the `install/ini.rs`
      editor it uses and the `install/configure.rs` dispatch every game shares
- [x] RBR: set `language` and `sounds` in `Plugins\Pacenote\PaceNote.ini`,
      preserving every other line; write from a built-in template only
      when absent; report `PluginMissing` when `Plugins\Pacenote\config\`
      is not there, which the caller localizes
- [x] The ini editor keeps the author's spacing on BOTH sides of the `=`,
      keeps an inline comment, never matches a commented-out line, keeps
      the file's CRLF, and adds a missing key to its own section rather
      than the end of the file
- [x] NOT a port of `CreateRBRConfiguration` — the style packages now ship
      the `config\pacenotes\*.ini` files it generated, and the voice
      packages ship a complete `PaceNote.ini`. Verified against the live
      catalogue
- [x] `sounds` = the voice's path below its category, checked against
      `Plugins\Pacenote\sounds\<that path>` on disk; if it is not there,
      keep what the package shipped and log it
- [x] Running twice writes nothing the second time, so a re-run takes no
      restore point and the ledger does not churn
- [x] `engine::record_configured` puts the step's edits back into the
      ledger. Found by auditing this list against the code: the step runs
      after extraction and edits what the packages wrote, so the recorded
      hashes went stale and the next uninstall read OUR edit as somebody
      else's and left the file behind. Two tests now pin both sides of
      that line
- [ ] A file the step writes that no package owns is logged, not adopted
      — the step knows it wrote the file but not what was there before,
      and claiming it would let a later uninstall delete something the
      user had all along. Consequence: when a voice package ships no
      `PaceNote.ini` and the step writes one from the template, that file
      stays after everything is uninstalled. To fix it properly the step
      has to report created-versus-edited, which means a shape change to
      `Configured`

### Per-game choice categories

DONE 2026-10-25, but NOT where this section first proposed. The list lives
in the compiled-in install SCHEMA (`schemas/<game>.toml` → `schema.rs`),
not in `gamedir.rs`: the same file the server publishes is what the
installer embeds, so the hash comparison that gates downloading and the
behaviour it protects come from one source instead of two that can drift.
`gamedir.rs` stayed what it is — which games exist and how to recognise
their folders.

- [x] `CHOICE_CATEGORIES` is no longer what the installer asks from:
      `schema::choice_categories(game)` returns the `select = "one"`
      categories in schema order. The localized label and explanation stay
      in the `.rc` string tables, mapped by category id in `wizard/scene/configuration.rs`,
      because UI text must not come from a server
      (`pacenotes_shared::CHOICE_CATEGORIES` survives for `shared`'s own
      tests and as the vocabulary constant)
- [x] Scene 4 demands a pick only in those; everything else is optional
- [x] Scene 5 asks one question per entry, in schema order, and a game
      whose schema declares none skips the scene
- [x] `install_root` is a per-game property, carried by the schema as a
      TOKEN (never a path, so a server cannot steer where files go), and
      `schema::install_roots_known` refuses a token this build cannot
      place
- [ ] What each game's token MEANS is still only decided for RBR (= the
      game folder). The other three are decided when that game gets
      packages (review P10 — BeamNG mods normally live under
      `%LOCALAPPDATA%`, not beside the exe)

### Restore (scene 3's Restore/Uninstall → the restore scene)

THE NEXT PIECE OF WORK. The writing half has been done since I7 and runs
on every install; the reading half does not exist, so the button says "not
implemented yet". What is already there to build on: `restore_dir` in `engine/execute/restore_point.rs`
(where a point lives), the `ledger.json` snapshot inside each point, the
replaced files beside it, and `Ledger::replace_all`.

- [x] A run that changes anything — not only one that REPLACES something —
      writes `.pacenotes\restore\<timestamp>\` holding `ledger.json` as it
      was before the run, and the files that run replaced. A run that only
      added files still needs a point, because undoing it means removing
      them. A run with an empty plan writes nothing
- [x] Point names are `2026-10-25T18-15-00Z`: sortable, and legal on
      Windows, which rules out real ISO 8601's colons
- [x] Points are never pruned automatically: they are small, and the one a
      user needs is the one a policy would have deleted
- [ ] The scene lists points newest first with their dates, and PREVIEWS
      the chosen one: files put back, packages removed, what survives
- [ ] Restore = put files back (oldest version wins across points) AND
      remove what was installed since, then replace the live ledger with
      the snapshot. Copying files back alone leaves a broken package's
      added files behind, which is the bug the legacy restore had.
      `Ledger::replace_all` exists for the last step; nothing else of this
      is written
- [ ] Planning it is pure and belongs in `engine.rs` beside `plan()`: a
      point plus the live ledger in, a list of put-backs and removals out,
      table-tested off Windows. The scene then shows that list as the
      preview and hands the same value to the executor, so what the user
      was shown and what runs cannot diverge
- [ ] It runs on the progress screen like any other phase, with the same
      log — a restore that half-finishes has to leave the same ledger
      guarantees an install does
- [ ] Reached from scene 3, so the Route enum grows a third arm and the
      page order grows a page. Back from it goes to scene 3; Cancel
      behaves as on scene 4b (before anything is written) and is disabled
      once writing starts

### Scene 6

- [x] THREE new phase labels, not two: Checking as well as Uninstall and
      Configure, because the pre-flight can take a visible moment on a
      large install. `worker::Phase`, the `IDS_S6_*` ids and both
      STRINGTABLEs; `Phase::of` maps the engine's enum onto the UI's, which
      are deliberately separate so `engine.rs` knows nothing about the UI
- [x] The removal confirmation before anything is touched (review P1),
      naming every installed package the selection no longer holds
- [x] Free space measured on the UI side with `GetDiskFreeSpaceExW` and
      handed to the engine as a value
- [x] **The download phase moved OUT of this scene (2026-10-26).** It is
      scene 4b now, and the configuration questions happen between the two.
      Why: a download fills a cache and a install writes to the game
      folder, and the user should be able to do the first without the
      second (fetch at home, install later) and the second without the
      first (change which co-driver is active, with the server down). It
      also makes the questions easier to answer honestly, since a list
      built from the cache cannot offer something that failed to download
      - `worker::DownloadJob`/`run_download` and `worker::InstallJob`/
        `run_install` are now two jobs; `Fetched` is the download's own
        outcome, because a finished download is not a finished run
      - Cancel belongs to 4b and is DISABLED here: every phase on this
        page writes to the game folder and the engine cannot stop between
        files, so the button could only lie
      - `Outcome::Done` no longer carries the download counts; the wizard
        keeps them, since on the Configure/Install route there were none

### Scene 7

- [x] Reports what actually changed: installed and removed counts, the
      files left alone because something else had changed them, and the
      restore point this run created
- [x] The headline is `IDS_S7_OK` now that the engine runs.
      `IDS_S7_PENDING_ENGINE` is unused and should be deleted from both
      string tables when something else touches them

### Crash reporting

- [x] **A panic now says where it happened** (2026-10-26). A GUI binary
      with `panic = "abort"` has nowhere to print, so a panic used to be a
      window that vanished plus a WER record pointing at an offset in a
      stripped exe — unusable to a user and nearly so to us. `main.rs`
      installs a hook that writes `panicked at <file>:<line>: <message>`
      into the run log, or into `pacenotes-installer-crash.log` beside the
      exe when no game folder has been settled yet, and shows it once
      - Only the FIRST panic opens a window: the modal pumps the message
        queue, which can deliver the very message that panicked, and one
        fault was seen becoming a cascade of boxes
      - It found the route-switch abort below on its first run
- [x] What the USER sees is localized and plain (decided 2026-10-26): what
      happened, where it was written, that the game folder is not
      half-installed, and "Show that file?" — which opens Explorer with the
      log selected. A report a user cannot find is one they cannot send.
      The Rust text stays in the file for whoever reads the report
      - Loaded after re-applying the chosen language
        (`i18n::use_chosen_language`), because the hook can run on the
        worker thread, which never had it set
- [x] `--selftest-panic [--out FILE]` and `tests/Test-CrashReport.ps1`: the
      crash path is the one piece of error handling that using the
      installer correctly never exercises, and an untested crash handler is
      one that fails the day it is needed. Asserts the exit code Windows
      reports, that the report names a source file and LINE, and that a
      panic with no run log still lands beside the exe
- [ ] NOT done, decided against for now (2026-10-26): a
      `SetUnhandledExceptionFilter` for access violations and other faults
      that never reach a Rust panic hook. Revisit if a crash ever appears
      that leaves no panic line — that is exactly the signature of one

### Tests

- [x] **Regression: switching routes mid-run** (2026-10-26). Configure/
      Install → change a pick → Back → Download/Update aborted the
      process. The Configure route loads a tree MODEL with no tree CONTROL
      to mirror it into; the control then sent an item-changed
      notification whose `lParam` was 0 — indistinguishable from "node 0"
      — the old guard (`node >= tree.len()`) passed because the model was
      no longer empty, and the posted refresh indexed a handle table that
      was still empty. Fixed three ways: the handle table is sized
      wherever the model is assigned, the refresh is driven off the handle
      table rather than the model's length, and a notification is
      validated by its `hItem`. Third pass of `Test-WizardDownload.ps1`
      walks the whole sequence and checks the crash log stayed empty
- [x] plan() table tests, 19: fresh install, no-op re-run, switch style,
      switch voice, exclusive_group supersede, changed sha, withdrawn
      package, unfinished install, every `requires` case
- [x] execute() in a tempdir, 14: unowned-file backup, an identical file
      NOT copied but still put back on uninstall, a file replaced by Steam
      left alone, a shared file kept while another owner remains, recovery
      from `installing`, a corrupt payload caught before anything is
      touched, free space, restore points
- [x] RBR step against the REAL shipped `PaceNote.ini`, 15: one line
      changes and the rest is byte for byte, the author's
      `muteReplayPacenotes` and comments survive, a missing file gets the
      template, a missing config dir is reported and nothing is written
- [x] End to end through the GUI, against a live server: the files are in
      the game folder, the ledger records them, a restore point exists,
      and the plugin settings carry the language that was picked
- [ ] Restore: to a point two runs back, with added files removed and the
      ledger matching the folder afterwards. Waits for the restore code

## Server protocol changes waiting on the installer (2026-10-25)

The server side is done; these are the installer halves. See root
`CLAUDE.md` "Protocol" and `server/TASKS.md` S10/S11/S12.

- [x] **Install schema check (S11, replaces "minimum manifest version").**
      `Manifest.schemas[<game>].sha256` is the SHA-256 of the server's
      `schema.toml` bytes. `build.rs` turns every `schemas/<game>.toml` into
      a static table (`schema.rs`, `include_bytes!` of the canonical file),
      hashed with `sha256.rs` — only the hash is compared, and nothing
      parses TOML at runtime. `schema::ALSO_ACCEPTED` holds the LIST of
      older hashes a build still takes, so a release can understand both
      schemas during a rollout. Unknown or missing hash for the chosen game
      → block DOWNLOADING for that game only, with "a newer installer is
      needed" (`IDS_S4_NEEDS_NEWER`); reconfiguring, repairing,
      uninstalling and restore keep working. The server's `categories` and
      `install_root` are never acted on — they are for the explanation in
      the log, not instructions.
- [x] The check runs only on a manifest that came FROM THE SERVER
      (`build_tree(.., from_server)`), so an offline run rebuilt from the
      cache never claims the installer is out of date.
- [x] Behaviour comes from the installer's OWN copy of the schema, which is
      also what makes the hashes meaningful: `schema::choice_categories`
      drives scene 4's gate and scene 5's lists, and the labels stay in the
      `.rc` tables. That replaced the per-game lists this file used to put
      in `gamedir.rs`.
- [x] A test that the compiled-in schema is byte-identical to
      `schemas/<game>.toml` on disk and hashes the same (`src/schema/tests.rs`,
      which reads the file at test time), plus one that its `select = "one"`
      categories are the ones the installer asks a single pick from, so the
      embedded hash and the compiled behaviour cannot drift apart. No `toml`
      dependency was needed, not even a dev one.
- [x] A CRLF test, because the hash is of RAW BYTES: `.gitattributes` marks
      `schemas/*.toml` `-text`, and a checkout that rewrote the line endings
      would make every installer built there reject every server.
- [x] Rewrite installer/CLAUDE.md "Minimum manifest version" and
      DESIGN.md scene 4's "If the server's manifest is newer…" to the
      schema model.
- [x] Versions may now read `sha-xxxxxxxx` (derived from the payload's
      sha256 when the operator sets none, D5). Nothing to do: no scene
      shows a version. The cache stores it (`CachedPackage.version`) and
      the engine compares shas, not version strings, so a derived version
      is already handled as-is. Revisit only if a scene starts showing
      one
- [x] Test fixtures: `Test-HttpClient.ps1` and `Test-WizardDownload.ps1`
      were given `schema.toml` files by the server change (a game without
      one now publishes nothing).

## I8 — Auth flow

- [x] Secret from scene 2 used as bearer for /packages (scene 4's fetch)
      and /download (scene 4b's `DownloadJob`). `/report/conflict` is the
      one request that deliberately carries no token
- [x] 401 on ACCEPT → warning modal (via `/whoami`, see below); a failed
      download → the message in the run log, the run ends at scene 7 in
      failure state rather than walking on to the questions
- [x] **Validate the secret with `GET /whoami`, not `/packages`**
      (server change 2026-10-25, DESIGN-REVIEW.md D2). `/packages` never
      401s — it falls back to the public listing for any key it does not
      accept — so `worker::check_secret` could never see `Rejected` and a
      mistyped key looked accepted. `/whoami` answers 200
      `shared::KeyStatus { expires_at }` for a live key and 401 for every
      bad one. `api::whoami` maps a 401 to `Rejected` and EVERYTHING else,
      including a 200 it cannot parse, to `Unreachable`: a protocol change
      must not lock a paying user out of their content.
- [x] `expires_at` is shown: a key within `EXPIRY_NOTICE_DAYS` (14) of
      expiry raises an informational modal naming the DATE, not a day count
      — Polish needs three plural forms for "days" and `load_string_arg`
      substitutes one `%s`. A key that lapses between two runs otherwise
      looks exactly like content being withdrawn.
- [x] The server no longer sends 403 (D3): a live key without the grant
      gets 404, the same as a missing package. Nothing in the installer
      handles 403, which is the correct amount — confirmed against the
      code and pinned by `Test-HttpClient.ps1`'s auth matrix.
- [ ] Optional: remember the secret and prefill scene 2. NOT a plain file
      as first sketched — the review (§7) asks for DPAPI
      (`CryptProtectData`, crypt32, no crate), so another user or a script
      reading the profile does not get the key. Keep it out of
      `%APPDATA%` too if it can live in the same per-user place as the
      cache, since the installer now writes everything else either there
      or in the game folder

## I9 — Polish

- [ ] All unwraps in UI paths replaced with modal/log + graceful handling.
      Less urgent than it was — since 2026-10-26 a panic names its file and
      line in the run log and tells the user — but a reported crash is
      still a crash
- [x] Free-disk-space check: measured on the UI side with
      `GetDiskFreeSpaceExW`, handed to the engine as `HostFacts.free_bytes`
      and enforced in `preflight` (`PreflightError::NotEnoughSpace`), which
      is a better place than "before scene 6" — the engine is what knows
      how much the plan actually needs
- [ ] Cache housekeeping: orphaned <sha>.zip sweep on startup; decide where
      "remove from cache" lives now that there is no library view
- [ ] Remember last game folder per game; prefill scene 3
- [ ] Keyboard/tab order, default buttons, Esc = Cancel behavior per scene

## I10 — Release gate
- [ ] tests/Test-BinarySize.ps1 < 600 kB on release build (the gate moved
      once, deliberately, when the install engine landed on 2026-10-25);
      record final size here: ____
- [ ] Bump `[package] version` in Cargo.toml AND the four VERSIONINFO fields in
      res/installer.rc together; `tests/Test-VersionSync.ps1` enforces it, and
      the CD workflows tag the release from Cargo.toml
- [x] cargo bloat report saved to `docs/bloat.txt` (2026-10-26, at
      584.5 kB). Where the space is: `.text` 387 kB, of which std 202 kB,
      our own code 100 kB, serde_core 34 kB and serde_json 14 kB. The
      other ~197 kB of the file is resources, and `rbr.ico` alone is 83 kB
      — more than serde and our engine put together, which makes it the
      cheapest room to buy
- [ ] If over budget: replace serde_json with minimal JSON parse/write for
      Manifest + cache.json + installed.json (planned fallback, CLAUDE.md §5);
      revisit the 83 kB icon (see I1 size flag)
- [ ] Decide what to do about `vcruntime140.dll`. The release binary imports
      it, so a machine without the VC++ 2015-2022 redistributable refuses to
      start it with a dialog that says nothing useful. Most gaming PCs have
      it, which is exactly why this would be found by the one user who does
      not. Either document the dependency (README does now) or build with
      `-C target-feature=+crt-static` and re-measure against the gate — a
      static CRT costs size, and the budget is the reason to decide rather
      than assume.
- [ ] Manual E2E on clean Windows VM, per game: full wizard online with and
      without secret, offline run from cache, switch style on re-run,
      failure path (server dropped mid-download) lands on failure summary.
      A CLEAN VM is also what settles the vcruntime question above
