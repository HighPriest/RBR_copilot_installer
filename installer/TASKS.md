# Installer tasks (I1..I10)

Specs are in `installer/requirements.md`, rules in `installer/CLAUDE.md`,
behaviour in `installer/DESIGN.md`. Test conventions are in
`tests/README.md` — read it before adding a test.

## I1 — Toolchain and window

- [x] Add the `x86_64-pc-windows-msvc` target
- [x] Compile `res/installer.rc` from `build.rs` via `embed_resource`
  - [x] Watch `../rbr.ico` too, or a new icon is not a reason to rebuild
- [x] Put the app manifest, icon and VERSIONINFO in the .rc
- [x] Use a plain `fn main` rather than `wWinMain`
- [x] Pin every dependency with `=`
- [x] Add `tests/Test-BinarySize.ps1` as the release gate

## I2 / I2a — Pre-wizard layouts

Superseded by I2b. Two layouts were built and removed; what survived them
is in the current design. See the CHANGELOG and git history.

## I2b — Wizard

### Protocol groundwork in `shared`

- [x] Derive the tree from the package's storage path: `category()`,
      `tree_levels()`, `leaf()`
- [x] Add `valid_name` as the choke point that stops a lookup key becoming
      a path, and `PackageRef::parse` splitting on the first separator
- [x] Add the remaining `GAME_*` constants
- [x] Add the category vocabulary

### Shell

- [x] Host the scenes in a PropertySheet, one dialog template each, with a
      per-scene primary label and a Cancel that confirms
- [x] Own `WizardState` at the sheet
- [x] Remove the superseded ribbon and two-column code and strings
- [x] Size every page 330x215 DLU, which scene 5 forces: three lists
      visible at once, each full width so a co-driver's path is not clipped

### Scenes

- [x] Scene 1 — language combo, defaulting to the OS UI language
- [x] Scene 2 — MOTD with an offline fallback, the secret field, and
      server status from `/health`
- [x] Scene 3 — path edit, Browse, and a game dropdown fed by a debounced
      scan
- [x] Scene 3 — three route buttons: Download/Update, Configure/Install,
      Restore/Uninstall
  - [x] Lay them out in two columns, the main flow alone on the right as
        the default button, wider and as tall as the other two together
  - [x] Measure that arrangement in `Test-UiLayout.ps1`, since "looks
        primary" is the whole point of it
  - [x] Leave the sheet's Next disabled: the routes are alternatives, not
        one primary and two afterthoughts
- [x] Scene 4 — the catalogue as a checkbox tree with partial states and
      parent/child propagation, falling back to the cache when offline
  - [x] Refuse Download until every choice category has a pick, naming the
        ones missing
- [x] Scene 4b — the download phase on a page of its own, between the tree
      and the questions
  - [x] Share scene 6's template and handlers: it is the same screen at an
        earlier moment
  - [x] End a failure or a cancel at the summary rather than walking on to
        questions about an install that cannot happen
- [x] Scene 5 — one single-select list per choice category, each option
      labelled with its path inside that category
  - [x] Preselect a sole option, and refuse Install until every list that
        has options has a pick
  - [x] Default to what is already installed, so the Configure route does
        not make the user re-answer last time's questions
  - [x] Carry picks across a trip Back to scene 4 and forward again
- [x] Scene 6 — phase label, progress bar, current package with its own
      byte count, and a live log pane
  - [x] No Back and no Next: the page runs itself and walks on
- [x] Scene 7 — result, game and folder, the picks, the extras, what came
      down the wire, and where the log is
  - [x] Hide the log pane on success and show it on failure
- [x] The restore scene — see "Restore" below
- [x] Add the run log (`runlog.rs`): in memory plus a line-by-line flush
  - [x] Write it to `<game dir>\.pacenotes\logs\install-<stamp>.log`, one
        file per run, newest five kept
  - [x] Attach it when scene 3 settles a folder, replaying what came
        before, so the file is the whole run
- [x] Write English and Polish strings for every caption, label and modal

### Models (no Win32, tested cross-platform)

- [x] `tree.rs` — manifest plus cache to nodes with states, check
      propagation and per-category validation
- [x] `choices.rs` — scene 5's lists, the path labels, preselection and
      the no-choice extras

## I3 — WinHTTP client

- [x] Wrap open/connect/request; add `get` and `get_streaming` with a sink
- [x] Read in 64 KiB chunks, reporting progress
- [x] Set explicit timeouts, or a dead server parks the worker and Cancel
      does nothing
- [x] Treat a body shorter than its Content-Length as an error
- [x] Turn WinHTTP error codes into readable text via winhttp.dll
- [x] Add `--selftest-http`, writing a report to a file or a message box
  - [x] Never write the token into the report

## I4 — Worker thread and progress protocol

- [x] Post a `Message` per job as a leaked `Box` the receiver reclaims
- [x] Spawn with `std::thread`; only the UI thread touches controls
- [x] Carry a request id, so a late answer to a superseded request is
      recognised and dropped
- [x] Support cancellation through a shared flag read between chunks
  - [x] Veto the window close until the worker reports back
- [x] Carry phase, item, per-item and per-phase byte counts, and an
      optional log line in the progress message
  - [x] Throttle posts to ten a second
- [x] Hand the job a `Reporter` rather than the window handle, so "a
      worker never touches a control" holds by construction

## I5 — API and cache

- [x] Write `sha256.rs` with no dependencies
  - [x] Test against the published vectors AND against chunked input
- [x] Add `fetch_manifest(game)`, `fetch_motd` and `health`
- [x] Lay the cache out at `<game dir>\.pacenotes\cache\`
  - [x] Open it when scene 3 settles a folder, as the log does
  - [x] Key `cache.json` by game as well, since directory+name is not
        unique across games
  - [x] Store a `PackageInfo` snapshot per entry, so the tree can show
        local-only packages offline
  - [x] Write the index atomically
- [x] Download to a temp file, verify the sha256, then move into place
  - [x] Build the URL with `PackageInfo::download_path()`, never by hand
  - [x] Reject a digest mismatch
- [x] Give each leaf a cache state, withholding an entry whose payload has
      vanished rather than failing at install time

## I6 — Zip reader

- [x] Read the EOCD and central directory; inflate and stored
- [x] Create the directories a .NET zip carries no entries for
- [x] List entries without extracting, for the backup phase
- [x] Return `/`-separated paths on every host: they are ledger keys
- [x] Guard against zip-slip, treating `\` as a separator and rejecting
      `:` outright
  - [x] Fail the package WHOLE, so nothing is half-installed
- [x] Check each entry's CRC32 during extraction
  - [x] Use a nibble table rather than the usual 256-entry one
- [x] Detect and refuse Zip64 by name rather than misreading it
- [x] Decode CP437 names without the UTF-8 flag, as .NET does
- [x] Test against real `Compress-Archive` output, deflated and stored
- [x] Add a manual check over a directory of real packages

## I7 — Ledger and engine

### Ledger (`ledger.rs`)

- [x] Read and write `.pacenotes\installed.json` atomically
- [x] Key entries by `game/directory/name`
- [x] Record per file: the path as written, a case-folded map key, the
      sha256 of what was written, and `prior`
- [x] Record `exclusive_group` as installed, so uninstall works offline
- [x] Add `owners_of(path)`, so a file is deleted only when no other entry
      lists it
- [x] Write `state: "installing"` BEFORE extracting
- [x] Recover in `plan()` rather than a startup scan: an `installing`
      entry is torn down and laid again
- [x] Never overwrite an unreadable ledger — it may be a newer build's

### Engine (`engine.rs`)

- [x] `plan()`: selection plus ledger plus cache to ordered actions
  - [x] Supersede a ledgered member of the same category or group
  - [x] Reinstall when the sha changed
  - [x] Enforce `requires` within the game, satisfied by the SELECTION
        alone — "already installed" does not count, since anything
        unselected is uninstalled in the same run
- [x] Uninstall before install, always
- [x] Pre-flight before touching anything: free space and a writability
      probe
- [x] Verify each payload by its sha256 rather than CRC-checking every
      entry
- [x] Write each file to a temp name and rename
- [x] Uninstall by `prior`, and only when the on-disk sha still matches
      what was written
- [x] Prune emptied directories, deepest first
- [x] Detect overlaps as a pure function over the file lists the
      pre-flight already read
- [ ] Measure "the game is not running" — the engine refuses when told,
      but `wizard/scene/installation.rs` still passes `false`
  - [ ] Check for the marker process (I9)

### Conflict reporting

- [x] Log every overlap, naming each writer in install order
- [x] POST the report after `execute()`, so what is reported is what
      happened
  - [x] Trim it to the server's 8 KiB cap by measuring, not guessing
  - [x] Convert keys to the refs the report carries, dropping any that
        will not split rather than guessing
  - [x] Send no token, no identity and no local paths, and say in the log
        that it was sent
- [x] Add `post_json` to `http.rs`, sharing one request path with `get`

### Per-game configuration step (`install.rs`)

- [x] `configure(game, game_dir, picks)` dispatching on the game, handed
      only a category-to-key map
- [x] RBR: set `language` and `sounds` in `PaceNote.ini`, preserving every
      other line, and write from a template only when the file is absent
  - [x] Report a missing plugin directory for the caller to localize
  - [x] Keep the author's spacing, inline comments, commented-out lines
        and the file's CRLF
  - [x] Check `sounds` against what is on disk, keeping what the package
        shipped when it is not there
  - [x] Write nothing on a second run, so a re-run takes no restore point
- [x] Put the step's edits back into the ledger
      (`engine::record_configured`), or the next uninstall reads our own
      edit as somebody else's
- [ ] Log a file the step wrote that no package owns, rather than adopting
      it — see requirements.md, "deliberately not done"

### Per-game choice categories

- [x] Take the list from the compiled-in schema rather than a constant
  - [x] Keep the localized label and explanation in the `.rc` tables,
        mapped by category id, because UI text must not come from a server
- [x] Demand a pick on scene 4 only in those categories
- [x] Ask one question per entry on scene 5, in schema order, and skip the
      scene when a game declares none
- [x] Carry `install_root` as a token the installer knows, never a path
  - [x] Refuse a token this build cannot place
- [ ] Decide what the token means for the three games other than RBR

### Restore

- [x] Write a restore point whenever a run changes anything, holding the
      pre-run ledger and the files that run replaced
  - [x] Name points by UTC stamp, sortable and legal on Windows
  - [x] Suffix the name when two runs share a second, rather than
        overwriting the first run's way back
  - [x] Never prune points automatically
- [x] List points newest first, each a date plus what the run after it did
- [x] Preview the chosen point: removals first, then put-backs, then what
      is left alone, then what cannot come back
- [x] Offer "remove everything this installer added" as a restore to an
      empty state — the same code, not a second path
- [x] Plan purely (`restore::plan`), and hand the worker the same value
      the preview showed
  - [x] Install by payload digest, not through the cache index, which
        holds only the newest version of each key
  - [x] Leave a package whose payload is gone installed rather than
        removing what cannot be replaced
  - [x] Write the snapshot's per-file provenance afterwards, or a later
        uninstall deletes a file the user always had
- [x] Run it on the progress screen like any other phase
  - [x] Take a restore point of its own first
- [x] Re-run the configuration step with the picks the restored state
      implies
- [ ] Re-download a payload the restore needs but the cache lacks — see
      requirements.md, "deliberately not done"

### Crash reporting

- [x] Install a panic hook that writes file, line and message to the run
      log, or beside the exe before a folder is settled
  - [x] Open a window for the FIRST panic only: the modal pumps the queue
        and can deliver the message that panicked
- [x] Show the user something localized and plain, offering to open the
      log
  - [x] Re-apply the chosen language first, since the hook can run on the
        worker thread
- [x] Add `--selftest-panic` and `tests/Test-CrashReport.ps1`

### Tests

- [x] `plan()` table tests: fresh install, no-op re-run, switching a pick,
      supersede, changed sha, withdrawn package, unfinished install, every
      `requires` case
- [x] `execute()` in a tempdir: unowned-file backup, an identical file put
      back on uninstall, a file replaced by Steam left alone, a shared file
      kept, recovery from `installing`, a corrupt payload, free space,
      restore points
- [x] The RBR step against the real shipped `PaceNote.ini`
- [x] End to end through the GUI against a live server
- [x] Restore: the bad-update rollback, the provenance proof, a missing
      payload, the point-name collision, and a full pass through the UI

## I8 — Auth flow

- [x] Use the secret as the bearer for `/packages` and `/download`
  - [x] Send no token with `/report/conflict`
- [x] Validate it with `GET /whoami`, not `/packages`
  - [x] Map 401 to rejected and everything else to "could not ask", so a
        protocol change cannot lock a paying user out
- [x] Warn when a key is within 14 days of expiry, naming the date rather
      than a day count
- [x] Handle no 403 at all, and pin that in the auth matrix
- [ ] Remember the secret and prefill scene 2 — see "Pre-release
      hardening"

## Server protocol changes

- [x] Check the install schema by hash (S11)
  - [x] Compile `schemas/<game>.toml` into a static table from `build.rs`
  - [x] Keep a list of older hashes a build still accepts, for rollouts
  - [x] Block downloading for that game only, and only when something has
        to come down the wire
  - [x] Run the check only on a manifest that came from a server
  - [x] Take behaviour from the installer's own copy of the schema
  - [x] Test that the compiled-in copy is byte-identical and hashes the
        same, and that it has no CRLF
- [x] Accept versions of the form `sha-xxxxxxxx` (D5) — nothing to do, as
      no scene shows a version
- [x] Give the test fixtures their own `schema.toml`

## Pre-release hardening

From the design review's period 2. Specs for each are in
`installer/requirements.md`; the findings are in `DESIGN-REVIEW.md`.
Ordered security first, then what support needs, then what a user hits.

- [x] **C1** Decide what happens on a redirect
  - [x] Find out whether WinHTTP re-sends `Authorization` across a host
        change, and record the answer
  - [x] Turn redirects off and follow them by hand: a hop limit, `https`
        only, and the header dropped when the host changes
  - [x] Refuse to redirect a request that carries a body, rather than
        choosing silently between re-sending it and making it a GET
  - [x] Pin it with `tests/Test-Redirects.ps1`, driving the real WinHTTP
        path rather than a reimplementation of it
- [x] **S1** Build the user agent from `CARGO_PKG_VERSION`
- [x] **§7** Give the run log a header and per-line timestamps
- [x] **D12** Try `/packages` even when `/health` failed
- [x] **D12** Cap what `http::get` will buffer
- [x] **P11** Probe writability on scene 3, offering to restart elevated
- [x] **§7** Retry a transient download, and list failures on scene 7
- [ ] **§7** Store the remembered secret with DPAPI

## I9 — Polish

- [x] Check free disk space (in `preflight`, which knows what the plan
      needs)
- [ ] Replace the unwraps left in UI paths with a modal and a log line
- [ ] Sweep orphaned payloads from the cache on startup
  - [ ] Decide where "remove from cache" lives now there is no library view
- [ ] Remember the last game folder per game and prefill scene 3
- [ ] Fix keyboard and tab order, default buttons, and Esc per scene

## I10 — Release gate

- [x] Save a `cargo bloat` report to `docs/bloat.txt`
- [ ] Pass the 600 kB gate on the release build, and record the final size
- [x] Version every build from its release tag, `v/X.Y.Z`, so no version
      is ever typed by hand
  - [x] `build.rs` reads the tag, refuses any other shape, and falls back
        to `0.0.0-dev` without one
  - [x] The user agent and the conflict report take that version
  - [x] VERSIONINFO comes from a header `build.rs` writes, so its four
        fields cannot drift
  - [x] `Cargo.toml`'s version stops meaning anything, and says so
  - [x] Release on pushing a `v/*` tag, retiring the merge and manual
        releases that read `Cargo.toml`
  - [x] Replace `Test-VersionSync.ps1` with a test that builds from a tag
        and reads the version back out of the binary
  - [x] Update DEVELOPMENT.md, the CLAUDE.md files and the test docs
- [ ] If over budget, pull a lever from `requirements.md`: the icon first,
      then a hand-rolled JSON parser
- [ ] Decide what to do about `vcruntime140.dll`: document the dependency
      or build with a static CRT and re-measure
- [ ] Manual end-to-end on a clean Windows VM, per game: online with and
      without a secret, offline from cache, switching a pick on a re-run,
      and a server dropped mid-download
  - [ ] The clean VM is also what settles the vcruntime question
