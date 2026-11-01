# installer/CLAUDE.md — Windows GUI installer

Windows-only (x86_64-pc-windows-msvc). HARD RELEASE GATE: final .exe < 700 kB.
Run `tests/Test-BinarySize.ps1` after every change that touches dependencies or adds
noticeable code. If size regresses, fix it before continuing.

The gate was 500 kB until 2026-10-25, when the install engine (I7) took
the binary to 546 kB, and 600 kB until 2026-10-31, when the installer
stopped needing the Visual C++ redistributable (I10, below). Each raise
measured the alternatives first — see "The size budget" below. It was 620
kB until 2026-11-01, when scene 5's readiness check (I9) landed the build
on exactly 620 kB and the maintainer raised it to 700; that raise came
with no measurement, and none of the levers below was spent. Treat 700
as the same kind of hard line 500 was: the answer to "we are over" is to
find the bytes, and the levers below are still on the table. The next
build that wants more should have to argue for it as these did.

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

## The size budget

Measured with `cargo bloat --release --target x86_64-pc-windows-msvc
--crates` (install it with `cargo install cargo-bloat`). At 546 kB, on the
day the gate moved:

| | |
|---|---|
| `std` | ~195 kB |
| this crate | 89 kB |
| the icon, in `.rsrc` | 83 kB |
| serde_core + serde_json + memchr + itoa | 43 kB |
| miniz_oxide | 2.7 kB |

Two findings worth keeping, because both are counter-intuitive:

- **miniz_oxide costs almost nothing.** It was budgeted at ~40 kB and is
  under 3. Inflate is a small algorithm; it was the engine around it that
  cost.
- **One `{:.1}` on an `f64` costs ~16 kB.** Formatting a float links Rust's
  whole float-to-decimal machinery. `size.rs` therefore does its one
  decimal digit with integer arithmetic, and nothing in this crate
  formats a float. Check before adding one.

**The second raise, 600 → 620 kB (2026-10-31), and its argument.** The
exe imported `VCRUNTIME140.dll`, the Visual C++ redistributable, which
Windows does not ship: on a PC without it, Windows refuses to start the
installer with an unlocalized "VCRUNTIME140.dll was not found", before a
single line of ours runs. Measured on the same commit:

| Build | Size | Needs the redistributable |
|---|---|---|
| dynamic CRT (as it was) | 595.5 kB | yes |
| fully static CRT | 692.5 kB | no |
| **hybrid CRT** (static VC runtime, Universal CRT from Windows) | 617.5 kB | no |

The icon lever (below) would have fitted the hybrid build under 600; the
maintainer chose to keep the icon and raise the gate to 620 instead. The
hybrid CRT is `+crt-static` in `.cargo/config.toml` plus two link
arguments from `build/runtime_link.rs`, which also REFUSES to build
without `+crt-static` — a `RUSTFLAGS` variable replaces the config file's
flags, so CI and `Run-AllTests.ps1` deny warnings through clippy instead.
The same file sets `[build] target`, so even a plain `cargo test` keeps
those flags off host tools: without it they reached the proc macros,
whose rebuild collided with the copies an editor's rust-analyzer holds open.
`Test-BinarySize.ps1` fails the build if the exe imports anything Windows
does not ship.

Levers still unspent, in order of cheapness, if the gate is threatened
again:

1. **The icon, ~85 kB for a single 256x256 PNG.** Windows downscales that
   one image for every size it needs, so the taskbar and list views look
   worse than they should AND it is the single largest object in the
   binary. Re-encoding losslessly saves only 4 kB — the image is that
   dense — but shipping 16/24/32/48/64/128 instead saves 39 kB and looks
   better everywhere except Explorer's extra-large view. Dropping to 64
   as the largest saves 66 kB and does look worse.
2. **serde_json, ~43 kB plus the derive code in this crate.** The planned
   fallback (point 5 above): a minimal hand-rolled parser/writer for the
   three documents we read. Keep JSON isolated in `api.rs`, `cache.rs` and
   `ledger.rs` so this stays a contained change.
3. The two `--selftest-*` developer flags, ~8 kB, behind a cargo feature —
   but CI drives `--selftest-http` against a live server, so the shipped
   binary and the tested one would stop being the same thing. Last resort.

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
**EVERYTHING this installer writes lives under the game folder it was
pointed at**, in that folder's `.pacenotes\`: the ledger, the backups, the
restore points, the run logs AND the download cache. Nothing goes into the
user's profile or anywhere else. Deleting the game takes every trace of
this tool with it, and a folder moved to another drive brings its
downloads and its way back along with it.

The cache lived in `%LOCALAPPDATA%` until 2026-10-27, which bought one
thing — two game folders sharing a download — at the price of state nobody
remembers to clean up. The trade was made the other way: disk is cheap,
and a second copy is what keeps the first folder's restore points working
after the second folder is deleted.

- **Cache** (`cache.rs`): `<game dir>\.pacenotes\cache\`
  - `<sha256>.zip` — the payloads, keyed by manifest sha (integrity+dedupe)
  - `cache.json` — index: (game, directory, name) → {sha256, version, size,
    updated_at, display_name, exclusive_group, requires} — a snapshot of
    PackageInfo, so the Download tree works fully OFFLINE. Category and the
    tree levels are NOT stored: they are derived from `name`.
  - Download = fetch → sha256 verify → move into cache. Re-download only when
    manifest sha differs from cached sha.
  - **Swept when it opens** (a route taken on scene 3): leftover `.part`
    files, and payloads named by NONE of the index, the ledger and every
    restore point (`restore::payloads_in_use`). NOT "what the index does
    not name": the index keeps one version per key, and the version an
    update replaced is exactly what a rollback needs. If any of those
    records cannot be read, no payload is deleted. A `--cache` override is
    never swept, since it may serve other folders.
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

    An `identical` original is only recoverable while that payload is in
    the cache, and nothing can guarantee it: a user can empty the cache by
    hand, and the orphan sweep does not read the ledger. Pinning is best
    effort. The rule that holds regardless: **if the original cannot be put back, the file is left
    alone and logged — never deleted.**
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
- **Facts the engine cannot measure are INJECTED, never called for.** Free
  disk space and "is the game running" arrive as values the caller
  measured (`HostFacts`), because `engine.rs` has to stay testable off
  Windows. Anything else that would need Win32 inside the engine belongs
  on that struct instead.
- **Install** (`engine.rs`): input = the wizard's result (the three
  Configuration picks plus the checked `pacenotes` packages) + the ledger.
  Plan = uninstall (ledgered but no longer chosen, including a ledgered
  member of a category or `exclusive_group` that a new pick supersedes) then
  install (chosen but not ledgered, or sha changed = update). Enforce
  `requires`. Scene 6 runs the plan, from the CACHE: the download phase is
  a screen of its own (4b) that happens BEFORE the configuration questions,
  so the install page never reaches the network except to report a conflict.
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
- **Choice categories are per game, and they come from the COMPILED-IN
  schema** — `schema.rs`, built by `build.rs` from `schemas/<game>.toml`.
  Not from `shared`, where one list drove scene 5 for all four games, and
  not from the manifest. A category with `select = "one"` is a question
  scene 5 asks; everything else is optional.
  - The schema says WHICH categories exist and which ask for one pick.
    The LABEL and EXPLANATION of each stay in the `.rc` string tables with
    every other string, mapped by category id in
    `wizard/scene/configuration.rs` (`category_caption`). Shipping UI text
    through the manifest would mean the server deciding what the installer
    says, in languages it cannot know; `install_root` is a token for the
    same reason — a server must never be able to steer where files go.
  - Only a game's choice categories are MANDATORY on scene 4. An operator
    adding an `extras/` folder must not turn it into a question every user
    has to answer.
  - A game with no choice categories skips scene 5 entirely.
  - `build/schema_table.rs` reads those files with a hand-rolled strict
    reader, not the `toml` crate: three string keys in one table array need
    no parser, and a schema using anything beyond that fails the BUILD with
    a line number instead of compiling a table that disagrees with the
    server's reading of the same file. Nothing parses TOML at runtime.
- **Install-schema check (replaces the earlier "minimum manifest
  version").** What an installer can fall behind on is the SHAPE of a
  game's catalogue, not the number of packages in it. So `Manifest.schemas`
  carries, per game, the SHA-256 of that game's schema file's RAW BYTES,
  and `schema::check` compares it with the hash of the copy this build
  embeds. A hash this build does not know means the catalogue may hold
  categories it cannot place. The rule for what happens then matters as
  much as the check:
  - It blocks DOWNLOADING, and only for THAT game. An out-of-date
    installer may still reconfigure, repair, uninstall and restore what is
    already on the machine — the user did not break anything by not
    updating, and taking their existing setup away from them would be the
    worst possible response. None of those paths consults the server.
  - The warning appears when the user tries to download, not at startup.
  - An offline run never shows it: the check runs only on a manifest that
    actually came from the server, never on one rebuilt from the cache.
  - The hash is of raw bytes, so `.gitattributes` marks `schemas/*.toml`
    `-text`. A checkout that rewrote the line endings would make every
    installer built there reject every server.
  - A revision that is NOT a real incompatibility (a fixed comment, a
    reordering) goes in `schema::ALSO_ACCEPTED` with a line saying why it
    does not matter — otherwise people learn to click through the prompt.
  See S1 in `DESIGN-REVIEW.md` and S11 in `server/TASKS.md`.

## Module map

- `main.rs` — entry point, common-controls init, the developer flags
  (`--server`, `--cache`), launches the wizard. `--cache` overrides
  `<game dir>\.pacenotes\cache` for development, to repeat a download that
  already happened once.
- `crash.rs` — the PANIC HOOK. A GUI binary with `panic = "abort"` has
  nowhere to print, so without it a panic is a vanished window; it writes
  `panicked at <file>:<line>` into the run log (or beside the exe before a
  folder is known) and shows it once, localized, offering to open the file.
  It found the Configure→Back→Download abort in one run.
  `--selftest-panic` and `tests/Test-CrashReport.ps1` keep it honest: a
  crash handler nothing exercises is one that fails when it is needed.
- `selftest/` — `--selftest-http`, the WinHTTP check that
  `tests/Test-HttpClient.ps1` drives, `--selftest-download`,
  `--selftest-report`, which posts a fabricated conflict report, and
  `--selftest-panic`.
- `elevation.rs` — P11's restart as administrator: the `Restart` a new
  instance resumes from (language, folder, key; it opens on scene 3), the
  UAC launch, and whether this process is already elevated. Its command
  line is tested against Windows' own splitter (`CommandLineToArgvW`), since
  a folder with spaces or a trailing backslash must come back exactly.
- `game_process.rs` — whether the game is running FROM THIS FOLDER
  (P12), by the process's own path, so a copy installed elsewhere does not
  block; unreadable counts as running. Feeds `HostFacts::game_running`,
  which the engine's pre-flight refuses on. Paths are compared by hand:
  `std::fs::canonicalize` cost 3.5 kB.
- `single_instance.rs` — one copy per user. A restarted instance waits for
  the one that launched it to close; the check closes the handle it gets
  for a name already taken, or that handle would keep the mutex alive.
- `run_header.rs` — the run log's header: build, start, server, and the
  Windows build from `RtlGetVersion`.
- `package_download.rs` — fetch a package into the cache, verified by sha256,
  retried after a failure a retry could fix (`HttpError::is_transient`, or
  a payload damaged in transit) with a cancellable wait. Scene 4b and
  `--selftest-download` both go through `download_retrying`, so the test
  drives the code the user runs.
- `wizard/` — the UI. Layout per `DESIGN.md`: a comctl32 PropertySheet in
  wizard mode (`sheet.rs`), one page per scene (`scene/`), with `WizardState`
  (`state.rs`) shared across pages and what every page does alike in `page/`.
  Control ids mirror `res/installer.rc` in `control_ids.rs`. Nine pages and
  THREE ROUTES, which part on scene 3 (`scene/game_folder_routes.rs`):
  Download/Update (4 → 4b fetches → 5 asks → 6 installs),
  Configure/Install (straight to 5, then 6, with no server and no
  transfers) and Restore/Uninstall (`scene/restore.rs`, then 6). Scenes 4b
  and 6 are the same screen at two moments, so they share their control ids
  and `scene/progress_page.rs`; only `Route` decides what is skipped. Before
  changing the sheet, read Implementation notes in `DESIGN.md`: the scene
  title, the per-scene button labels and the title bar all work around
  something the OS does not give.
  - The restore page is registered LAST in the sheet, because page order is
    what "Next" means: between the questions and the progress page, Install
    walked onto it. It is reached and left by id.
  - INVARIANT: `WizardState.tree_items` (node index → `HTREEITEM`) is
    assigned wherever `tree` is, with a zero handle for a node the control
    does not hold. SysTreeView32 sends an item-changed notification with
    `lParam = 0`, indistinguishable from node 0, before it holds anything
    of ours, and acting on it aborted the Configure route. So the two are
    sized together, the refresh is driven off the handle table, and a
    notification is validated by its `hItem`.
  - INVARIANT: whatever replaces `tree` with anything but scene 4's own
    catalogue clears `catalogue_loaded`, so scene 4's next visit fetches
    and rebuilds the model and the control together. The Configure route
    once left them disagreeing: Download then passed its gate with nothing
    ticked, and the next visit ignored every tick.
  - Scene 5 is entered only through `configuration::has_something_to_pick`
    (on scene 3 for Configure, after 4b for Download): every choice
    category needs a checked package the cache HOLDS (`Cache::holds`).
- `ui/` — the small Win32 vocabulary the pages are written in: control
  text, combo boxes, list boxes, the three modal shapes, the folder picker,
  and `paint_forward_button`, the light green of the way forward. A themed
  push button ignores the colours its parent offers, so it is drawn whole
  from `NM_CUSTOMDRAW`: Next and Finish through a subclass of the sheet
  (`wizard/sheet.rs`, hooked at `PSCB_INITIALIZED`), since they are the
  sheet's controls, and Download/Update from scene 3's own page.
- `choices.rs` — scene 5's model: the three single-select lists and the
  no-choice extras, derived from what scene 4 committed. Pure.
- `tree/` — scene 4's model: manifest + cache → rows, states, checks. Pure.
- `runlog.rs` — the run log: in memory for the pane, flushed line by line to
  `<game dir>\.pacenotes\logs\install-<stamp>.log`, newest five kept. The UI
  thread owns it, as it owns the controls. It is attached once scene 3
  settles a folder, replaying what was said before, because the log belongs
  with the install it describes. The FILE opens with a header (build, start,
  server, Windows build; `run_header.rs` in the bin) and times every line in
  UTC; the pane shows neither. `utc_time.rs` is the one place a date is
  formatted — do not hand-roll another.
- `size.rs` — byte counts as a person reads them ("41.6 MB"), in binary units
  so they match Explorer.
- `worker.rs` — the one background thread. A worker NEVER touches a control;
  it posts a `Message` and the UI thread acts on it.
- `gamedir.rs` — which games exist, their dropdown labels, and the marker
  executable that identifies each one's folder, plus `detect()` which drives
  the wizard's game dropdown. Pure std, so the folder check is testable
  without a desktop. A game folder is NOT "any directory that exists"; see the
  note under Testing.
- `http/` — WinHTTP wrapper: `get`/`get_streaming` with a progress callback,
  and `post_json` for the one thing the installer sends
  (`/report/conflict`), over one request path. Synchronous; always called
  from the worker thread.
  - `get` holds the whole body in memory, so it refuses one over 8 MiB
    (`MAX_BUFFERED_BYTES`), counting rather than trusting `Content-Length`,
    which a chunked reply does not send. A small document is the server's
    promise, and the cap is what holds when it is broken. Anything big —
    a package — goes through `get_streaming` into a file.
  - **REDIRECTS ARE FOLLOWED HERE, NEVER BY WINHTTP, and the bearer token
    never leaves the origin the caller asked for.** Measured 2026-10-30:
    left to itself WinHTTP follows a 3xx and re-sends `Authorization` to
    the new origin while this code never sees the hop — so a 302 from a
    misconfigured CDN or a hijacked record would hand over the user's
    access key silently. Each request sets
    `WINHTTP_OPTION_REDIRECT_POLICY_NEVER` and `send` loops itself: at most
    `MAX_REDIRECTS` hops, no https→http downgrade, and the token attached
    only while scheme, host and port are all unchanged. Do not "simplify"
    this back into WinHTTP's own redirect handling.
    `tests/Test-Redirects.ps1` is what keeps it honest.
- `api.rs` — the endpoints: `/packages`, `/motd`, `/whoami`, `/health`,
  `/download/...` into the cache, and `POST /report/conflict`; JSON is
  isolated here. `/whoami` is the ONLY way to validate an access key, since
  `/packages` answers the public listing for a key it does not accept. A
  401 means wrong, unknown, expired or revoked; anything else, a 200 we
  cannot parse included, is "could not ask", which per `DESIGN.md` carries
  on with the key unvalidated.
- `schema.rs` — the install schemas this build embeds (generated by
  `build.rs`), the per-game choice categories read off them, and
  `check()`, the one comparison made against the server's schema hash.
- `cache.rs` — cache dir layout, cache.json read/write, sha verification.
- `ledger.rs` — installed.json read/write (atomic), file ownership queries.
- `engine.rs` — plan + execute the install; backup; restore points; uses
  zip.rs. `uninstall_package` and `install_package` are `pub(crate)` because
  `restore/` drives the same steps; `install_package` takes an
  `Installing` descriptor rather than looking the package up, since the
  cache index holds only the newest version of each key and a restore
  wants an older one.
- `restore/` — the other half of a restore point: reading them back
  (`point.rs`), planning what going to one would mean (`plan.rs`, pure,
  table-tested), and carrying that out through the engine's steps
  (`execute.rs`). "Uninstall
  everything" is a restore to an empty state, not a separate path. Three
  rules it exists to keep: the plan is a value computed before anything
  moves, so the preview and the work cannot disagree; nothing is removed
  unless what replaces it is ready; and the ledger written afterwards
  keeps the snapshot's per-file `prior`, because that — not the bytes —
  is what stops a later uninstall deleting a file the user always had.
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

- **No `unwrap`, `expect` or `panic!` outside tests**, and clippy enforces
  it: `lib.rs` and `main.rs` deny `clippy::unwrap_used`, `expect_used` and
  `panic` when not compiling tests. A panic in a GUI built with
  `panic = "abort"` is a vanished window. The one deliberate panic,
  `--selftest-panic`, carries an `allow` saying why; build scripts are a
  separate crate and fail the BUILD with a message on purpose.
- Every user-reachable failure → `MessageBoxW` with a localized
  string, then either recover or clean exit. No silent catch-and-continue
  (the C# did this; don't copy it).
- A failed install must leave the ledger truthful: ledger updates are per-file
  batch per package, written after that package's action succeeds.

## Size audit workflow

- `tests/Test-BinarySize.ps1` — fails build over 700 kB (release gate), or
  on an import Windows does not ship.
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
- `tests/Test-ReleaseVersion.ps1` — a release's version comes from its
  `v/X.Y.Z` tag and ONLY from there: `build/release_version.rs` reads
  `PACENOTES_RELEASE_TAG` and feeds the user agent, the conflict report and
  all four VERSIONINFO fields (via a generated header), and fails the build
  on any other shape; no tag means `0.0.0-dev`. The test builds from tags
  and reads the version back out of the binary. Never type a version into
  `Cargo.toml` or the `.rc` — nothing reads them.
- `tests/Test-BinarySize.ps1` — the 700 kB release gate, and the imports check.
- `tests/Test-HttpClient.ps1` — builds and runs a real `pacenotes-server`,
  then drives `--selftest-http` against it: /health, anonymous vs
  authenticated /packages, downloads, 401, 404, an unreachable port, and
  that the server's access log names this build's user agent.
  WinHTTP cannot be unit tested, so this is the only thing that proves
  `http.rs` works. Needs no desktop, so CI runs it.
- `tests/ui/Test-UiLayout.ps1` — AUTOMATED TEST that drives the real .exe
  through user32.dll. Run it in BOTH languages (`-Language 0` / `-Language 1`)
  after ANY change to `res/installer.rc`, `ui.rs` or `i18n.rs`. The Polish pass
  is what proves the per-language STRINGTABLE lookup and the UTF-8 diacritics.
  It also makes a folder this account cannot write (a deny rule set
  through .NET, so no admin rights are needed), checks scene 3 stops with
  the right message and leaves no trace, and launches with the restart
  arguments while the first instance still runs. It answers the
  restart question NO (`Decline-Modal`): yes would raise a real UAC prompt.
  And the keyboard (`Send-Key`, `Get-FocusedControlId` in `Win32Ui.psm1`):
  where focus starts on scenes 1–3, scene 3's Tab order, and that Enter in
  the folder field takes Download/Update. Only the dialog manager decides
  those, so `Send-Key` POSTS the key into the wizard's queue, where the
  sheet's loop hands it to the dialog manager; a message SENT to a control
  would skip it. Not real input (`keybd_event`): that goes to whatever window
  is in front, Windows' foreground lock would not reliably let a background
  test put the wizard there, and a missed key lands in another application.
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
test`, the release-version test and the size gate — everything except the GUI tests.
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
