# Changelog

## 2026/10/31 — installer: a failed /health no longer hides the catalogue (D12)

One failed `GET /health` on scene 2 made the whole run offline: scene 4
built its tree from the cache without asking for the catalogue, although
`/packages` would have answered — a load balancer hiccup, a `/health`
slower than the rest. It did the same when the user simply reached scene 4
before `/health` had answered at all.

Scene 4 now always asks for the catalogue; `/health` decides only scene 2's
status line. When both fail the status still says the server could not be
reached, when only the catalogue fails it says the list could not be read,
and a catalogue that answers after `/health` did not is written to the run
log, since that mismatch is what support would want to see.

`Test-WizardDownload.ps1` gained a fifth pass that builds exactly that
situation: a stand-in in front of the real server fails `/health` and relays
everything else. Scene 2 reports Offline, scene 4 shows the whole catalogue
and says it is ready (103/103).

## 2026/10/31 — installer: the run log says when, which build and where (§7)

Support asks the same three things before anything else: when did this
happen, which build was it, and which server was it talking to. The run log
answered none of them.

- Each log FILE now opens with a header — `Pacenotes Installer <version>`
  (the release tag's), the UTC start time, the server URL and the Windows
  build — then a blank line.
- Every line in the file carries its time, in UTC and in the shape the
  server's log writes (`2026-10-31T12:11:31Z  …`), so the two can be lined
  up. A line said before the game folder was known keeps the time it was
  said when it is replayed into the file; the crash hook's line is timed too.
- The Windows build comes from `RtlGetVersion`. `GetVersionExW` would have
  said Windows 8: the manifest names no newer Windows, so it is told the
  compatibility answer.
- The pane on scenes 6 and 7 is unchanged: no header, no times.
- Dates are formatted in one place, `utc_time.rs`, which the restore point
  names and the key-expiry notice now use as well.

`Test-WizardDownload.ps1` checks the header and the times on the real file
the wizard wrote (98/98). 582 kB against the gate.

## 2026/10/31 — installer: the release tag is the version

A release used to start by typing the same version into five places —
`installer/Cargo.toml` and four VERSIONINFO fields in `res/installer.rc` —
which the workflows then read to CREATE a tag. It now runs the other way:
**pushing `v/X.Y.Z` is the release**, and that tag is the only place a
version is written.

- `cd-release-on-tag.yml` builds on a pushed `v/*` tag and passes it to the
  build as `PACENOTES_RELEASE_TAG`. `build/release_version.rs` turns it into
  the user agent, the conflict report's version and all four VERSIONINFO
  fields (through a header the `.rc` includes), and FAILS THE BUILD on any
  other shape — a mistyped tag cannot become a release. The workflow then
  checks the built `.exe` carries the tag's version before publishing.
- Without a tag a build is `0.0.0-dev`, so a developer's build is obvious in
  the server's access log and in the file properties.
- The release on merge to `master` and the manual release are gone: both
  read `Cargo.toml` and created the tag from it. Merging publishes nothing.
- `Cargo.toml` says `version = "0.0.0"`, with a comment that the tag decides.
- `Test-VersionSync.ps1`, which checked that the five hand-typed copies
  agreed, is replaced by `Test-ReleaseVersion.ps1`, which builds from real
  tags and reads the version back out of the binary — file properties and
  user agent — and checks that nine malformed tags (`v1.2.3`, `v/1.2`,
  `v/1.2.3-rc1`, `v/65536.0.0`, an empty one, …) each fail the build.

This supersedes the S1 entry below in one respect: the user agent's version
now comes from the tag, not from `Cargo.toml`.

## 2026/10/31 — installer: the user agent names the build (S1)

The installer announced itself as `Pacenotes-Installer/0.1` whatever its
version, so the server's access log could not tell one build from another.
It is now `Pacenotes-Installer/<version>`, taken from `Cargo.toml` at
compile time, so it moves with the version bump that tags a release.
`Test-HttpClient.ps1` checks it where it matters: in the real server's
access log, against the version in `Cargo.toml`.

That closes the installer's share of S1. The rest of the finding was
settled elsewhere: the manifest version and minimum installer were
superseded by the per-game schema hash (S11), which already tells an
out-of-date installer to update; the server has logged the agent since D11;
and `generated_at` / `generation` wait for the server's `/ready` (S6).
The conflict report already carried the same version, so the two agree.

## 2026/10/30 — installer: a bearer token no longer leaks across a redirect

The design review suspected it (C1, "probably with the `Authorization`
header"). It is now measured, and it was true: **left to itself WinHTTP
follows a 3xx and re-sends `Authorization` to the new origin**, while
`http.rs` never sees the hop at all. Two local listeners on two ports, the
first answering 302 to the second: the second received `Bearer <token>` in
full, and the client reported a clean `RESULT: OK` with the second's body.
Nothing logged it, because nothing knew.

That is a real leak, not a theoretical one. A 302 from a misconfigured
CDN, a hijacked DNS record, or an operator moving downloads without
thinking about the header would hand over the user's access key to a host
they never agreed to talk to.

So the decision is taken away from WinHTTP. `send` now sets
`WINHTTP_OPTION_REDIRECT_POLICY_NEVER` and follows redirects itself:

- at most three hops, so a redirect loop ends in milliseconds rather than
  on the timeout;
- no https→http downgrade — refused, not followed;
- the token attached only while scheme, host AND port are unchanged, and
  dropped the moment any of them is not;
- the 3xx body is never fed to the sink: it belongs to a response the
  caller is not getting, and would otherwise land inside a package file;
- a request with a body refuses to redirect at all, rather than silently
  choosing between re-sending the report and turning it into a GET.

A redirect to the same origin still works, so a server that rewrites paths
does not break auth, and a redirect to a CDN for public content still
works — what cannot happen is the key going with it.

`tests/Test-Redirects.ps1` pins it, driving the real WinHTTP path through
`--selftest-http`: cross-origin drops the token, same-origin keeps it, and
a loop gives up by itself. It needs no desktop and no server, so CI runs
it. The https→http refusal is deliberately NOT covered — an
`HttpListener` on loopback cannot easily serve real TLS and `http.rs`
lives in the bin, out of reach of `tests/` — and that gap is written down
in `installer/requirements.md` rather than left to be assumed.

## 2026/10/27 — installer: nothing is written outside the game folder

The download cache moves from `%LOCALAPPDATA%\PacenotesInstaller\cache` to
`<game dir>\.pacenotes\cache`, beside the ledger, the backups, the restore
points and the logs. That was the last thing this installer wrote anywhere
but the folder it was pointed at.

- Deleting the game folder now removes every trace of the tool, and a
  folder copied to another drive or PC arrives complete — with its
  downloads and its restore points, which is what makes the way back still
  work there.
- The cost, accepted deliberately: two game folders no longer share one
  cache, so a package wanted in both is fetched twice. Disk is cheaper than
  state nobody remembers to clean up — and the second copy is what keeps
  the first folder's restore points working after the second is deleted.
- `cache::default_root` is gone; `cache::dir_for(game_dir)` replaces it, and
  the wizard opens the cache when scene 3 settles a folder, exactly as the
  run log is attached. `--cache <dir>` stays as a development override.
- An audit of every write the installer makes found nothing else outside
  the game folder. What remains elsewhere is deliberate and is not install
  state: the `--selftest-*` developer flags use a throwaway temp directory,
  and a panic that happens before any folder is chosen writes its report
  beside the exe, because at that moment there is no game folder to put it
  in.
- The UI suite no longer passes `--cache`, so it exercises the real default,
  and it asserts both halves: the downloads land in the game folder, and
  nothing under `%LOCALAPPDATA%` is touched during a run.

Older versions left downloads in `%LOCALAPPDATA%\PacenotesInstaller`.
Nothing reads that folder any more; it can be deleted, and the README says
so.

## 2026/10/27 — installer: the restore scene

The last functional gap. Every install has written a restore point since
I7 — the ledger as it was, plus the files that run replaced — and nothing
could read one back, so users were accumulating a safety net they could
not use. Scene 3's Restore/Uninstall button now opens a scene that lists
those points newest first, previews what going back to one would do, and
does it.

- The list is a date plus what the run after it did (`+4 -0 ~0`), derived
  by diffing consecutive snapshots: nothing records what a run was about,
  and a column of identical dates is not something anyone can choose from.
  Signs rather than words, so the line needs no Polish plural forms.
- The last entry, **"Remove everything this installer added"**, is a
  restore to an empty state — the same code, not a second path, and the
  other half of the button's name.
- The preview leads with removals, because that is the part to read before
  agreeing. The PLAN it renders is the value handed to the worker, so what
  runs cannot differ from what was approved.
- A restore takes a restore point of its own: a user who restores to the
  wrong state must be able to come back from that too.
- Afterwards the game's configuration step runs again with the picks the
  restored state implies, or the files would come back without the
  language and voice the user had chosen.

Three things that cost more code than the happy path:

- **It cannot reuse `engine::plan`.** That asks the cache INDEX which
  version it holds, and the index holds one per key: the newest. Rolling
  back an update means naming the older payload, which is still on disk
  because payloads are sha-keyed. `install_package` now takes an `Installing`
  descriptor instead of looking the package up.
- **Never make it worse.** A package whose payload is gone from the cache
  is not removed either — the wrong version beats an empty folder — and
  the preview names it before the user commits. Everything else is still
  restored.
- **Restore the provenance, not just the bytes.** The snapshot records what
  was at each path before this installer first wrote there. A fresh
  install of the old version records `prior: absent` for a file it just
  deleted and rewrote, so a later uninstall would DELETE a file the user
  always had. The ledger written afterwards takes the snapshot's entries,
  and the end-to-end test restores, then uninstalls, and checks the user's
  own file comes back.

Found while testing, and fixed: **two runs in the same second shared a
restore point directory**, the second overwriting the first's snapshot and
saved files — silently destroying the way back the first run had promised.
Points are now suffixed (`…31Z-2`) when the name is taken.

Also: `rbr.ico` was ONE 256x256 frame costing 85 kB of a 600 kB budget,
more than serde and the whole install engine together, which Windows then
downsampled to 16x16 for the title bar. It is now 16/32/48 as BMP plus 128
as PNG, 45 kB, built by `installer/res/Build-Icon.ps1` from the source art
— smaller AND sharper at the sizes actually used. `build.rs` watches the
icon now; it did not, so a new icon was not a reason to rebuild and the old
bytes stayed in the binary.

Tests: 13 planning tests off Windows, 8 against a real game folder
(including the bad-update rollback and the provenance proof), and a fourth
pass in the wizard suite that restores through the real UI and checks the
folder afterwards. 90/90 there, whole suite green, 576.5 kB against the gate.

## 2026/10/26 — installer: scene 3's three routes, and a crash that hid itself

- **Scene 3's buttons are now arranged by importance, in two columns**:
  Download/Update is the main flow, so it is the default button, the widest,
  and as tall as the other two together, alone on the right;
  Configure/Install and the new **Restore/Uninstall** are secondary work on
  a folder you already have, so they stack in a narrower column on the left,
  top and bottom aligned with the big button so the three read as one block.
  `Test-UiLayout.ps1` measures the arrangement — column, stacking order,
  relative width, matching height — because "looks primary" is the point of
  it and nothing else would catch a regression.
  - Restore/Uninstall answers "not implemented yet" until the restore scene
    lands. It ships early so the page's shape does not change under users
    later, and so someone looking for a way back can see one is coming.
- **Fixed: Configure/Install → change a pick → Back → Download/Update killed
  the installer.** The Configure route loads a tree MODEL without a tree
  CONTROL to mirror it into. SysTreeView32 then sends an item-changed
  notification carrying `lParam = 0` before it holds anything of ours —
  indistinguishable from "node 0" — and the old guard (`node >= tree.len()`)
  no longer caught it, because the model was no longer empty. The posted
  refresh then indexed a handle table that was still empty and the process
  aborted. Fixed in three places, each of which would have been enough:
  - the handle table is sized wherever the model is assigned, so the two
    cannot drift;
  - the refresh is driven off the handle table rather than the model's
    length;
  - a notification is validated by its `hItem`, not by an lParam that an
    item we did not insert also has.
  The third pass of `Test-WizardDownload.ps1` now walks the whole sequence.
- **A panic now says where it happened.** This is a GUI binary with
  `panic = "abort"`: a panic had nowhere to print, so the window simply
  vanished and Windows recorded an offset into a stripped exe. A hook in
  `main.rs` writes `panicked at <file>:<line>: <message>` into the run log —
  or into `pacenotes-installer-crash.log` beside the exe before a game
  folder is known — and shows it once. Only the first panic opens a window:
  the modal pumps the message queue, which can deliver the very message that
  panicked, and one fault was observed becoming a cascade of boxes. It found
  the bug above on its first run.
  - What the USER sees is localized and plain: what happened, where it was
    written, that the game folder is not half-installed, and "Show that
    file?", which opens Explorer with the log selected. A report the user
    cannot find is one they cannot send. The Rust text stays in the file.
  - Loaded after re-applying the chosen language: the hook can run on the
    worker thread, which never had it set.
  - `--selftest-panic` and `tests/Test-CrashReport.ps1` exercise the crash
    path, which using the installer correctly never does. They assert the
    exit code Windows reports, that the report names a source file and
    line, and that a panic before any game folder is settled still leaves a
    report beside the exe.
  - NOT done, and deliberately: a `SetUnhandledExceptionFilter` for access
    violations and other faults that never reach a Rust panic hook. Worth
    revisiting if a crash ever appears that leaves no panic line.
- Taking a second route in one session no longer starts a second log file.
  One file per run, not per button press.
- Release build 580 kB against the 600 kB gate.
  48/48 layout checks per language, 76/76 flow checks, 7/7 crash checks.

## 2026/10/26 — installer: two routes, and the log moves into the game folder

- **The run log lives in the game folder**: `<game dir>\.pacenotes\logs\
  install-<utc stamp>.log`, beside the ledger, the backups and the restore
  points it describes. Nothing of ours is written to the user's profile any
  more except the download cache, which is shared by every game folder.
  - One file per run, newest five kept. A single overwritten file meant that
    re-opening the installer to see what went wrong erased the run being
    looked at (DESIGN-REVIEW §7 asked for at least three).
  - The file is attached when scene 3 settles a folder, and the lines from
    before that — language, server, catalogue — are replayed into it, so the
    file is the whole run rather than its second half.
- **Downloading and installing are now two screens with the configuration
  step between them.** Scene 3 offers two routes:
  - **Download/Update** → 4 (pick) → 4b (fetch) → 5 (configure) → 6
    (install) → 7.
  - **Configure/Install** → straight to 5, then 6. No server, no transfers:
    the questions are answered from the cache and the ledger.
  - Why: a download fills a cache, an install writes to the game folder.
    Someone can fetch a voice pack at home and install it on a train, and a
    user who only wants to change which co-driver is active never has to
    wait for — or depend on — the server. That is the common case after the
    first run.
  - The two routes are in-page buttons with the sheet's Next disabled on
    that scene: they are alternatives, not a primary and an afterthought,
    and each is one click rather than select-then-Next.
  - `Route` decides only what is SKIPPED. Both routes end in the same
    questions and the same install, reading the same tree, so there is no
    second "offline" code path to keep in step.
- Scene 4b is scene 6's template and handlers at an earlier moment. Cancel
  belongs to it — a half-finished download costs nothing — and is now
  DISABLED on scene 6, where every phase writes to the game folder and the
  engine cannot stop between files; the button could only have lied.
- `worker`: `DownloadJob`/`run_download` and `InstallJob`/`run_install` are
  two jobs, with `Fetched` as the download's own outcome, because a finished
  download is not a finished run. `InstallJob` carries no token and no
  package list: it installs from the cache and reaches the network only to
  report a conflict.
- Found by the UI suite as soon as the second route existed: **scene 5 now
  defaults to what is already installed.** A route whose purpose is changing
  an install must not ask every question blank — and a selection that
  omitted an installed package would have uninstalled it. The user's own
  pick still wins over the default.
- Tests: the flow suite walks both routes end to end in one run (68/68),
  asserting the ordering that the split exists for — when the questions are
  asked the payloads are cached and the game folder has no ledger yet — and
  that the second run keeps the first run's log. 44/44 layout checks per
  language. Release build 573 kB against the 600 kB gate.

## 2026/10/25 — installer: the client halves of S10, S11 and D2

Three protocol changes landed on the server the same day; these are the
installer sides of them. One commit, because they interleave in
`wizard.rs`, `worker.rs` and `api.rs` and splitting them would mean
commits that do not build.

- **Install schema (S11).** `build.rs` turns every `schemas/<game>.toml`
  into a static table (`schema.rs`: the file's bytes via `include_bytes!`,
  plus each category's `id`/`select`/`install_root`). Read with a
  hand-rolled strict reader, not the `toml` crate — three string keys in
  one table array need no parser, nothing parses TOML at runtime, and a
  schema using more than that fails the BUILD with a line number.
  - The schema now DRIVES behaviour: `schema::choice_categories` (the
    `select = "one"` categories, in file order) decides what scene 4
    demands a tick in, what scene 5 asks, and the tree's category order.
    Labels stay in the `.rc` tables, mapped by category id.
  - `schema::check` compares the manifest's per-game schema hash with the
    embedded copy's. A hash this build does not know blocks DOWNLOADING
    for that game only, with `IDS_S4_NEEDS_NEWER`; reconfiguring,
    repairing, uninstalling and restore keep working. `ALSO_ACCEPTED`
    holds hashes of revisions that are compatible anyway, so a rollout
    need not force an update for a corrected comment.
  - Only checked against a manifest that came FROM THE SERVER, so an
    offline run rebuilt from the cache never claims to be out of date.
  - `src/schema/tests.rs` reads `schemas/rbr.toml` off disk at test time and
    asserts the compiled-in copy is byte-identical, hashes the same, has
    no CRLF, and that its single-pick categories are the ones the
    installer asks a single pick from. The embedded hash and the compiled
    behaviour cannot drift apart.
- **Key validation via `/whoami` (D2).** `/packages` never 4xxs — it
  answers the public listing for a key it does not accept — so the old
  check could never fail and a mistyped key looked accepted. `api::whoami`
  maps 401 to `Rejected` and everything else, including a 200 it cannot
  parse, to `Unreachable`: a protocol change must not lock a paying user
  out. `KeyStatus.expires_at` is used, not ignored — a key within 14 days
  of expiry raises an informational modal naming the DATE (a date, not a
  day count: Polish needs three plural forms and `load_string_arg`
  substitutes one `%s`).
- **Conflict reports (S10).** `http.rs` grew `post_json`, sharing one
  request path with `get_streaming`. `api::report_conflicts` posts
  `shared::ConflictReport` after the install — so what is reported is what
  happened — with no token, no identity and no local paths, trimmed to the
  server's 8 KiB cap by measuring (`fitting_conflicts`) rather than
  guessing. `Overlap::refs` converts the engine's `game/directory/name`
  keys to the `directory/name` refs the report carries and DROPS any key
  it cannot split, since the server discards bad entries silently. A
  failed report is a log line; the user's install succeeded either way.
- `--selftest-report <game>` posts a fabricated report through that same
  code, and `Test-HttpClient.ps1` now asserts on the JSONL line the server
  stored (refs in install order, no `pcn_`, the server's own timestamp)
  plus the whole `/whoami` matrix: no key, unknown key, live key, and a
  key minted `--expires-days 3`. 49/49 against a live server.
- Fixed by the suite: `Test-WizardDownload.ps1` still demanded a tick in
  `pacenotes`, which the schema marks `select = "any"`. The test was
  stale, not the gate — an operator adding a folder must not add a
  question every user is forced to answer. 54/54.
- Release build 566 kB against the 600 kB gate.

## 2026/10/25 — server+shared: conflict reports from installers (S10)

- `POST /report/conflict`, no auth, body `shared::ConflictReport`
  `{game, installer, conflicts: [{file, packages}]}` — one entry per shared
  file, its writers in install order (last = winner). Revised from the flat
  lists first sketched, which lost which packages fought over which file.
- Treated as hostile input: 8 KiB cap (413), `game`/`installer`/each file
  and ref validated, bad entries dropped and counted (400 only when none
  survive), nothing echoed back, empty 202 on success.
- Rate limited per client (10/h) and globally (600/h) with `Retry-After`,
  checked before the body is parsed. Behind Caddy the peer is always Caddy,
  so the client comes from `X-Forwarded-For` — believed ONLY from a
  `--trusted-proxy` (`PACENOTES_TRUSTED_PROXIES`, IP or CIDR); a forged
  header from anyone else changes nothing (tested).
- Stored as JSONL in `data/reports/conflicts-YYYY-MM-DD.jsonl` with the
  server's `received_at`; no IP, key, hostname or local path is written.
- `manifest gen` warns per package pair reported in the last 30 days.
- Found by the suite: tokio's `File::write_all` may return before the bytes
  reach the OS, so the 202 could precede the write; appends now flush.
- `deploy/README.md` documents the reports, what they contain, and the
  trusted-proxy setting (`10.0.0.0/24` on this host).

## 2026/10/25 — server+shared: per-game install schema (S11)

Replaces the planned "manifest version". Decided in review: what an
installer can fall behind on is the SHAPE of a game's catalogue — its
categories, which ask for one pick, where each installs — not the package
list.

- `schemas/rbr.toml` (repository, canonical, byte-exact via
  `.gitattributes -text`, ASCII-only) declares RBR's four categories.
  Deployed as `<data>/<game>/schema.toml`.
- `shared`: `Manifest.schemas: { game: GameSchema { sha256, categories } }`.
  `sha256` is the hash of the file's RAW bytes — what installers compare.
  All fields are plain strings so a newer server can only fail an old
  installer's comparison, never its parse. `for_game` filters schemas too.
- `manifest gen` is strict: no schema → nothing published for that game;
  unparsable → same; a package in an undeclared category → skipped. The
  server bundles the repository schemas and warns when a data root's copy
  differs ("installers from this release will refuse"). `motd` and
  `reports` are reserved data-root names.
- Upgrade guard: `manifest gen` REFUSES (exit 1, nothing written) when a
  game the current manifest serves would vanish because its schema is
  missing — otherwise the first restart after upgrading would empty the
  live catalogue. Rehearsed against a copy of the live data root: refused
  without the schema; with it, all 24 packages published under hash
  `b9c5d124…`.
- Installer test fixtures (`Test-HttpClient.ps1`, `Test-WizardDownload.ps1`)
  now ship schema files; not run here (Windows-only).
- Installer side recorded in `installer/TASKS.md`: embed the schema and
  compare hashes (a list per game, for rollouts), block downloading only.

**Before redeploying the live server:** `cp schemas/rbr.toml
/home/user/pacenotes/data/rbr/schema.toml`, then run `manifest migrate-to-sidecars`
(`deploy/README.md`, "Upgrading").

## 2026/10/25 — server: the manifest is reproducible (DESIGN-REVIEW D5)

- `manifest gen` no longer reads the previous manifest: the catalogue is a
  pure function of zips + sidecars (+ schemas, next). Deleting a sidecar
  key now actually removes it, and two replicas built from the same tree
  publish identical manifests.
- A package without a sidecar `version` gets `sha-<first 8 hex of its
  sha256>` instead of the fixed `0.0.0`, so two payloads never share a
  version.
- Safety net for the switch: `manifest gen` REFUSES (exit 1, nothing
  written, old manifest keeps serving) when a run would drop a value the
  current manifest carries, naming each one; `--accept-metadata-loss`
  overrides for a deliberate removal.
- `manifest migrate-to-sidecars`: one-off move of manifest-only values
  into sidecars (adds missing keys, never overwrites, never pins `0.0.0` or
  a derived version, idempotent). Run once when upgrading
  (`deploy/README.md`).
- `deploy/README.md` no longer calls the packages "reproducible" — they are
  operator content with no other copy; the manifest now genuinely is.

## 2026/10/25 — server: observability and HTTP hygiene (DESIGN-REVIEW D11, D12, C2)

- **D11** One INFO line per response on the `access` target: method,
  path+query, status, bytes, ms, user agent, and the key as its 12-char
  HASH prefix (`-` none, `malformed` junk) — matchable against
  `token list`, never the key. Production runs at INFO, where the old
  `TraceLayer` logged nothing. Caddy's HTTPS block gains `log`.
- **D12** `Content-Disposition` names the leaf (`Numeric Standard.zip`, not
  `style/Numeric Standard.zip`), with an ASCII `filename` that cannot break
  its quotes and an RFC 6266 `filename*` carrying the exact UTF-8 name.
- **C2** `/packages` is `private, no-store` + `Vary: Authorization` (it
  differs per key); protected downloads `private, no-store`; public ones
  `no-cache`. Every download has `ETag: "<sha256>"` and answers
  `If-None-Match` with 304 — only after authorization.

## 2026/10/25 — server: robustness (DESIGN-REVIEW D10, S4, S5, S7)

- **D10** The server handles SIGTERM, not just ctrl-c — as the container's
  PID 1 it previously ignored `podman stop` until SIGKILL. In-flight
  requests drain for at most 20 s. Verified live: idle stop 0.01 s; a stop
  with a 250 s download in flight ends at 20.0 s. The Quadlet sets
  `StopTimeout=30` so podman waits for that drain.
- **S5** `manifest.json` is written atomically (temp + rename). A broken
  sidecar or a stray zip now withholds only that package: the rest still
  publishes and `manifest gen` exits **2** with `ERROR:` lines (1 = nothing
  written). The entrypoint prints a banner instead of a one-line echo.
- **S7** Local auth DBs run in WAL mode with a 5 s `busy_timeout`, so
  `token mint` during traffic cannot 500 a user.
- **S4** (minimal) A package whose size on disk differs from the manifest
  is answered 503 "being updated", not streamed as a short body that fails
  the client's sha check as "corrupted".

## 2026/10/25 — server: auth correctness (DESIGN-REVIEW D2, D3, D8, D9)

- **D3** Download statuses no longer enumerate. On a non-public directory, a
  caller with no live key gets 401 whether or not the package exists; a
  live key without the grant gets 404, identical to a missing package.
  **403 is gone.** The installer's live-server test (anonymous protected
  download = 401) still holds.
- **D2** `GET /whoami`: 200 `shared::KeyStatus { expires_at }` for a live
  key, 401 for none/malformed/unknown/expired/revoked. `/packages` can never
  answer "is this key valid?" (it falls back to the public listing), so the
  installer's scene-2 check was dead code; it moves to `/whoami`
  (`installer/TASKS.md` I8).
- **D8** Token rows decode fail-closed. Found while writing the test:
  libsql 0.6's typed `row.get::<i64>` PANICS on a type mismatch rather than
  erroring, so drift would have killed the request. Decoding now goes
  through `get_value` with explicit matching.
- **D9** Public downloads never touch the auth DB; `/packages` serves the
  public listing when the DB fails; a bearer value not shaped like a minted
  key is rejected before any query. Proven by a test that drops the auth
  tables mid-run.
- 401 responses now carry `WWW-Authenticate: Bearer`.

## 2026/10/13 — deployable server (deploy/)

Everything needed to put a real server in front of the installer for
testing:

- `deploy/Containerfile` — multi-stage build (rust:1-slim → debian-slim),
  binds `0.0.0.0:8080` because the CLI's loopback default would be
  unreachable from Caddy, and pins `CARGO_BUILD_JOBS=1` for the 3 GB host.
- `deploy/entrypoint.sh` — `manifest gen` then `serve`, so publishing a
  package is "copy the zip in, restart the unit". A failed generation leaves
  the previous manifest serving rather than taking the server down.
- `deploy/pacenotes.container` + `pacenotes.config.env` — Podman Quadlet
  following the host's `example.container` pattern (clone branch → build →
  discard checkout), with one volume (`~/pacenotes/data` → `/data`), a
  healthcheck on `/health`, and no published port: Caddy is the only route
  in and terminates TLS. Validated with podman's own quadlet generator.
- `deploy/Caddyfile.snippet` — `pacenotes.example.com` reverse-proxying to
  `pacenotes:8080`, plus the optional internal `http://` block.
- `deploy/README.md` — configuration and deployment walkthrough: data
  layout, publishing, sidecars, MOTD, access keys, installer test commands,
  operations table and troubleshooting.
- `libsql` lost its `replication` feature (tonic/tonic-web/prost-build/
  libsql_replication): embedded replicas are not something this server
  creates, and the dependency tree was the bulk of the image build.
  `core` + `remote` + `tls` keep Turso-by-`--db-url` intact. Tests unchanged
  and green.

Two departures from the `example.container` pattern, both deliberate:
`podman image prune -f` without `--external` (the `--external` form deletes
concurrent builds' intermediate containers — see below), and
`StartLimitBurst=3`/`StartLimitIntervalSec=1h` so a crash cannot loop a
release build.

## 2026/10/12 — one server, every game

Decided with the maintainer: one instance serves all games, and the first
folder level under the data root names the game.

- Layout is now `<data>/<game>/packages/<directory>/<name>.zip`, with
  `<data>/motd/<lang>.txt` beside the games and one `<data>/manifest.json`
  covering all of them. `serve`/`manifest gen` take `--data`
  (`PACENOTES_DATA_DIR`, default `./data`) in place of `--packages`.
- `PackageInfo.game` comes from that folder, so `game` is no longer a
  sidecar key: a sidecar still carrying it fails the run. Nothing positional
  comes from a .toml any more.
- `GET /packages?game=<id>` filters to one game; without it the document
  covers every game. An unknown game is an empty list, not an error.
- `GET /download/{game}/{directory}/{*name}`: the game leads the address,
  because two games can hold the same directory+name — without it the route
  would have silently served the wrong game's bytes. All three segments stay
  lookup keys; a miss on any is 404.
- `shared` gained `valid_id` (one rule for games and access directories),
  `KNOWN_GAMES`, `Manifest::for_game` and `PackageInfo::download_path`.
- `manifest gen` warns on the easy mis-filings (category where an access
  directory belongs, category/`public` where a game belongs, game folder
  with no `packages/`, unknown game id, name needing percent-encoding).
- The real `server/data` tree was re-filed under `public/`: 24 RBR packages
  now publish with correct categories, and a 28 MB nested download plus a
  percent-encoded name (`style/Numeric%20Standard`) were verified over HTTP.

INSTALLER SIDE, not done here: the download URL gained a leading game
segment, `/packages` can be filtered by game, and a cache/ledger key of
"directory/name" is no longer unique across games — use
`PackageInfo::download_path()` or prefix the game.

## 2026/10/12 — server side of protocol rev. 2

- `manifest gen` recurses: a package's `name` is now its path under the
  access directory without `.zip`
  (`packages/public/codriver/polish/male/Kamil.zip` →
  `public` + `codriver/polish/male/Kamil`), so the operator's folder layout
  is the installer's Download tree. Names are validated with
  `shared::valid_name`; rejected or non-UTF-8 names are skipped with a
  warning instead of entering the manifest.
- `GET /download/{directory}/{*name}` is a wildcard route so nested names
  match. The security invariant is untouched: both halves stay manifest
  lookup keys, only a hit yields a path on disk, and every miss — including
  percent-encoded traversal — is 404.
- `GET /motd?lang=xx` serves `<packages>/motd/<lang>.txt` as text/plain,
  falls back to `en`, 404s when absent (the installer treats that like being
  offline) and needs no credential. `lang` is restricted to `[a-z]{2,8}`
  before it touches the filesystem.
- `manifest gen` now warns when a top-level folder is a known category name,
  i.e. when the access-directory level has been left out — the mistake the
  real `server/data` tree had.
- Sidecars lost their last positional key: `game` aside, nothing about where
  a package sits comes from a .toml any more.

## 2026/09/20 — M3: server auth

- libSQL auth store: `db.rs` opens `--db-url` (local file, `:memory:`, or
  remote Turso by URL with `PACENOTES_DB_AUTH_TOKEN`) and applies
  `migrations/0001_init.sql` idempotently on every start.
- `auth.rs`: bearer parsing, sha256 hashing, minting (`pcn_` + 32 random
  bytes, hand-rolled unpadded base64url — no dependency for 20 lines), and
  a single-query `tokens LEFT JOIN grants` load. `TokenRow::is_live` is the
  one definition of "not revoked, not expired", shared by `decide` and the
  `/packages` filter.
- `/packages` filters to public + granted directories and never 4xxs (a bad
  credential silently sees the anonymous listing, so directory names cannot
  be enumerated). `/download` enforces `decide()`: 401 missing/expired/
  revoked, 403 valid-but-not-granted, 404 no such package — resolved before
  the access check, so a nonexistent package is 404 for everyone.
- `token` admin CLI: `mint` (prints the key once, stdout), `grant`,
  `ungrant`, `revoke`, `list`. Credentials are addressed by raw key or hash
  (full or the short form `list` prints); `public` and invalid directory
  names are refused; an ambiguous hash prefix is an error.
- `server` split into lib + bin so `server/tests/api.rs` drives the real
  router: the {anon, no-grants, dirA, dirA+dirB} x {public, dirA, dirB}
  matrix for listing AND download, plus expired, revoked, ungrant-takes-
  effect, traversal and ApiError bodies. 35 tests green, clippy -D warnings
  clean.
- Verified live: minting, listing, refusal to grant `public`, per-grant
  filtering, 401/403/200 on download, revoke taking effect — and the log
  contains neither the key nor the Authorization header.

## 2026/09/20 — M1a (protocol v2) + M2 (server MVP)

- `shared`: protocol v2. `PackageInfo` gains `game`, `category`,
  `display_name`, `exclusive_group`, `requires`; new `PackageRef`
  ("directory/name" — the download path, cache key, ledger key and `requires`
  format in one shape), `PackageInfo::label()/required_refs()`,
  `Manifest::get()`. Old (v1) manifests still deserialize via serde defaults,
  and absent optionals are not serialized, so the JSON stays hand-readable.
  `game` is not in the written M1a list: `installer/DESIGN.md` made the app
  multi-game, and without it every package would appear under every ribbon
  tab.
- `server`: M2 complete. `GET /packages` (public-only for now) and
  `GET /download/{directory}/{name}` streaming via `tokio_util::io::ReaderStream`;
  `manifest gen` scans `<dir>/<directory>/*.zip`, hashes each zip and merges
  its `<name>.toml` sidecar (precedence: sidecar > previous manifest >
  protocol default). A malformed sidecar aborts the run; dangling `requires`,
  sidecars without a zip, public-requires-protected and cross-game
  dependencies are collected warnings.
- Verified end-to-end with curl: health 200; anon listing shows only `public`;
  download byte-identical with correct Content-Length/Type/Disposition;
  unknown package and `..%2F..%2Fdata/auth.db` both 404.
- KNOWN GAP: `/download` is not access-checked yet (M3/S6). `/packages` hides
  protected directory names, but that is concealment, not access control —
  do not deploy before M3.

## 2026/09/18 — M0/M1 done (server side)

- All workspace dependencies pinned to exact versions; `clap` gained the
  `env` feature so the `PACENOTES_*` fallbacks compile. `cargo build`,
  `cargo clippy -D warnings` and `cargo test` are green for shared+server.
- GitHub Actions ported from `legacy/.github/workflows` (C#/dotnet → Rust):
  - `ci.yml` (new): fmt + clippy + test for shared/server on Linux,
    installer test + release build + 500 kB size gate on Windows.
  - `cd-debugbuild-manually.yml`: manual debug artifact (exe + pdb).
  - `cd-deploy-manually.yml`: manual release — version from
    `installer/Cargo.toml`, tag-exists check, size gate, GitHub release.
  - `cd-deploy-on-pull-request.yml`: same, on merged PR to master.
  Supabase secret injection dropped (no Supabase); the archived
  `actions/create-release` + `upload-release-asset` pair replaced with
  `softprops/action-gh-release`.

## 2026/09/18  

There was an error with the installer package.

```Powershell
error: current package believes it's in a workspace when it's not:
current:   E:\Git\RBR\CoPilot_InstallerV2\installer\Cargo.toml
workspace: E:\Git\RBR\CoPilot_InstallerV2\Cargo.toml

this may be fixable by adding `installer` to the `workspace.members` array of the manifest located at: E:\Git\RBR\CoPilot_InstallerV2\Cargo.toml
Alternatively, to keep it out of the workspace, add the package to the `workspace.exclude` array, or add an empty `[workspace]` table to the package's manifest.
```

Patched with addition of "workspace.exclude" in top-level `Cargo.toml`
