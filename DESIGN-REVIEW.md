# Design review: operational ergonomics, pitfalls, footguns

Date: 2026-10-24. Reviewed at commit `06dfc91` on `master`.

Scope: root, `installer/` and `server/` `CLAUDE.md` files, `installer/DESIGN.md`,
the three `TASKS.md` files, `deploy/`, `.github/workflows/`, and all code in
`shared/`, `server/src/` and `installer/src/`.

No code was changed. Findings marked **Reproduced** were confirmed by running
the existing `pacenotes-server` debug build against a scratch data tree. The
rest come from reading the code and the documents.

Severity:

- **Critical**: silent data loss, or a core feature that does not work.
- **High**: wrong behaviour that users or the operator will run into.
- **Medium**: a latent problem, or friction in day-to-day operation.
- **Low**: polish.

Each finding has an ID. D = a defect in the current code. P = a pitfall in the
install model. S = server, protocol or publishing. C = multi-server or cloud.
O = deployment and CI. U = support and observability.

---

## 1. Summary

**This document is the review, not the todo.** The findings below are kept
as written, with how each was resolved; the work that is still OPEN lives
in the task lists, where work is picked up:

| Still open | Tracked in |
|---|---|
| Installer, before release | `installer/TASKS.md` → "Pre-release hardening" |
| Deploy, CI and supply chain | `TASKS.md` → M7 |
| Multi-server (period 3) and the backlog | section 10 of this file |

Status, last checked 2026-10-27: "Fixed" means the code changed and a test
covers it.

| ID | Finding | Severity | Status |
|---|---|---|---|
| D1 | Packages marked "(update)" are never downloaded, so a republished package never reaches anyone who has the old one cached | Critical | **Fixed** |
| P1 | The tree ticks packages based on the per-user **cache**, but Apply compares against the per-folder **ledger**. Anything installed but not cached gets uninstalled silently | Critical | **Fixed** (I7) |
| P3 | Skipping the backup for byte-identical files makes the installer the owner of files the user already had. Uninstalling then deletes them, and there is no backup to restore | Critical | **Fixed** (I7) |
| P6 | If an install fails partway, the next run backs up the half-written files as if they were the user's originals | Critical | **Fixed** (I7) |
| D2 | An invalid access key can never be detected, because `/packages` never returns 401 | High | **Fixed** both sides |
| D3 | `/download` reveals which protected packages exist (401 vs 404). **Reproduced** | High | **Fixed** |
| D5 | Metadata removed from a sidecar stays in the manifest. A rebuilt manifest differs from the published one, and replicas drift apart. **Reproduced** | High | **Fixed** |
| P2 | Uninstall deletes ledgered files even after Steam, RSF or the user has replaced them | High | **Fixed** (I7) |
| P5 | Ledger paths are compared byte-for-byte, but NTFS ignores case | High | **Fixed** (I7) |
| C1 | Redirecting downloads to object storage or a CDN would break the current client: WinHTTP follows redirects, probably with the `Authorization` header | High | Open — see the table above |
| S4 | Publishing is not atomic: a download can stream a half-copied zip, and replacing a zip in place makes every client fail its sha check | High | Minimal fix **done** (size mismatch → 503); blobs still Period 3 |
| O1 | Every restart, including recovery after a crash, rebuilds the image from the branch tip. Publishing content therefore also deploys code | High | Open — see the table above |
| D10 | The server never shuts down gracefully in the container. SIGTERM is ignored (PID 1), and podman force-kills it after its stop timeout | Medium | **Fixed** |
| D4 | `manifest gen` reports false cross-game `requires` warnings because `Manifest::get` ignores the game. **Reproduced** | Medium | **Fixed** |
| D6 | `Cache::save` deletes the old file and then renames. A crash in between empties the cache index, and the planned orphan sweep would then delete every download | Medium (High once I9 lands) | **Fixed** |
| D8, D9 | The auth row decoder fails open. A DB error returns 500 even for public downloads | Medium | **Fixed** |
| D11 | Production has no access logs at all | Medium | **Fixed** |

---

## 2. Defects in the current code

### D1: Updates are never downloaded (Critical)

- **Where:** [installer/src/tree.rs:56-61](installer/src/tree.rs#L56-L61),
  [tree.rs:366-372](installer/src/tree.rs#L366-L372) and
  [wizard.rs:1163](installer/src/wizard.rs#L1163).
- **What happens:** `LeafState::is_present()` returns true for
  `UpdateAvailable`, and `checked_needing_download()` skips every leaf that is
  "present". As a result, a leaf marked "(update)" is ticked but never
  fetched. The cache index keeps the old sha, so once I7 exists it will
  install the old bytes again. There is no way for the user to force a
  refresh.
- **Test:** the test in [installer/tests/tree.rs:298-303](installer/tests/tree.rs#L298-L303)
  says "so it must refetch", but it only asserts that `is_present()` is true.
- **Fix:** split the predicate in two: `has_payload()` for "installable
  offline", and `needs_download()` for `NotCached | UpdateAvailable`. Add a test
  asserting that an update leaf shows up in `checked_needing_download()`.

### D2: An invalid access key cannot be detected (High)

- **Where:** [installer/src/worker.rs:217-227](installer/src/worker.rs#L217-L227),
  [server/src/routes.rs:56-78](server/src/routes.rs#L56-L78) and
  [installer/DESIGN.md:53](installer/DESIGN.md#L53).
- **What happens:** scene 2 validates the key by calling `GET /packages` and
  treats a 401 as "invalid secret". The server never returns 401 there: by
  design, any bad, expired or revoked key gets the public listing. So
  `SecretOutcome::Rejected` is dead code. A mistyped key looks accepted, and
  the user just sees public content with no explanation. The same thing
  happens when a valid key expires.
- **Fix:** add a `GET /whoami` endpoint, or a header on `/packages`, returning
  `{ valid, expires_at }` for the presented key. This tells a caller nothing
  they could not already know: a 256-bit key cannot be guessed, and the
  response names no directories. It also lets the installer warn a few days
  before a key expires.

### D3: `/download` reveals which protected packages exist (High, Reproduced)

- **Where:** [server/src/routes.rs:90-102](server/src/routes.rs#L90-L102), and
  the opposite claim in [CLAUDE.md:122](CLAUDE.md#L122).
- **Reproduced:** with no credential,
  `/download/rbr/premium/codriver/Secret` returned **401**, while
  `/download/rbr/premium/codriver/Nope` and `/download/rbr/nosuchdir/x`
  returned **404**.
- **Why it matters:** an anonymous caller can enumerate both directory names
  and package names. That undoes the reason `/packages` filters silently.
- **Fix:** for a directory other than `public`, return 404 to any caller
  without a grant for it. If you want to keep a distinct signal, return 401
  when there is no live token at all, whether or not the package exists. Then
  update the integration matrix in `server/tests/api.rs`.

### D4: False cross-game `requires` warnings (Medium, Reproduced)

- **Where:** [server/src/store.rs:508](server/src/store.rs#L508) and
  [shared/src/lib.rs:305-309](shared/src/lib.rs#L305-L309).
- **What happens:** `Manifest::get(&PackageRef)` matches on directory and name
  only. When both `beamng` and `rbr` publish `public/language/polish`, an `rbr`
  package that requires it resolves to the `beamng` one, because `beamng`
  sorts first. `manifest gen` then warns
  `(game rbr) requires public/language/polish (game beamng)`. The
  public-vs-protected check runs against the wrong package too.
- **Footgun:** the I7 engine will need `requires` resolution, and
  `Manifest::get` is the obvious helper to reach for.
- **Fix:** replace it with `Manifest::get(game, &PackageRef)`, or resolve refs
  within `package.game` only. Document that `requires` refs are scoped to the
  package's own game.

### D5: Metadata cannot be removed, and the manifest cannot be rebuilt (High, Reproduced)

- **Where:** [server/src/store.rs:327-359](server/src/store.rs#L327-L359).
- **Reproduced:** after `Kamil.toml` (which held `requires = [...]`) was
  deleted and `manifest gen` was run again, the manifest still carried the
  `requires`.
- **What follows:**
  - Removing `requires`, `exclusive_group`, `display_name` or `description`
    takes an undocumented step: deleting `manifest.json`.
  - Deleting `manifest.json` resets every version that was not in a sidecar
    to `0.0.0` and drops every display name that only existed in the old
    manifest. [deploy/README.md:235](deploy/README.md#L235) calls the
    manifest "reproducible", but it is not.
  - Two replicas that generate their own manifests from different histories
    publish different metadata.
- **Fix:** make the manifest a pure function of zips plus sidecars. Make
  `version` either required, or derived (for example from `updated_at` or a
  short sha). A one-off `manifest migrate-to-sidecars` command can write the
  current sticky values into sidecars once.

### D6: Crash window when saving the cache index, then the sweep deletes everything (Medium, High once I9 lands)

- **Where:** [installer/src/cache.rs:52-59](installer/src/cache.rs#L52-L59) and
  [cache.rs:109-123](installer/src/cache.rs#L109-L123).
- **What happens:** `save()` deletes `cache.json` and then renames the temp
  file into place, citing "Windows rename fails onto an existing file". That
  is not true for Rust: `std::fs::rename` on Windows replaces the target
  (MoveFileExW with `MOVEFILE_REPLACE_EXISTING`). The delete only opens a
  crash window. `Cache::open` treats a missing or unparseable index as empty,
  so every payload becomes an orphan, and the planned I9 sweep deletes
  orphans.
- **The same happens when** a newer installer writes an index that an older
  one cannot parse.
- **Fix:**
  - Drop the `remove_file` and rename straight over the old file.
  - Add `"version"` to `Index`.
  - Let `open` tell three cases apart: missing, unreadable and parsed.
  - Never sweep after an unreadable index. Only sweep orphans older than N
    days.
- The ledger (I7) is meant to copy this temp-and-rename pattern, so fix the
  pattern first.

### D7: No single-instance guard (Medium)

- **Where:** [installer/src/api.rs:102-106](installer/src/api.rs#L102-L106).
- **What happens:** the `.part` file is named after the package. The comment
  says this stops two runs from sharing a temp file, but two instances
  downloading the same package write the same `.part`. `cache.json.tmp` is
  shared as well, and `cache.json` is last-writer-wins. Once I7 exists, two
  instances aimed at one game folder will also race on `installed.json`.
- **Fix:** a named mutex (`CreateMutexW`, costs nothing in size), checked at
  startup with a "the installer is already running" modal.

### D8: The auth row decoder fails open (Medium)

- **Where:** [server/src/auth.rs:133-139](server/src/auth.rs#L133-L139).
- **What happens:** the code reads
  `revoked: row.get::<i64>(3).unwrap_or_default() != 0`, and `expires_at`
  falls back to `None` when the read fails. After a schema or type drift, a
  revoked key reads as live and an expiring key reads as permanent.
- **Fix:** an authorization path should fail closed: propagate the error and
  return 500.

### D9: A DB outage breaks public downloads (Medium)

- **Where:** [server/src/routes.rs:14-26](server/src/routes.rs#L14-L26),
  [routes.rs:68](server/src/routes.rs#L68) and [routes.rs:97](server/src/routes.rs#L97).
- **What happens:** `caller()` looks up the token before `decide()` for every
  package, public ones included. If the DB errors (a remote Turso blip, or a
  locked SQLite file), any request that carries a bearer token gets 500, even
  for public content or the public listing.
- **Fix:**
  - Skip the lookup when `package.directory == "public"`.
  - On `/packages`, fall back to the public listing on a DB error and log it
    loudly.
  - Reject tokens that are not shaped like `pcn_` plus 43 characters before
    querying, so garbage headers cost no DB round trip. On remote Turso,
    every query costs money.

### D10: Shutdown is never graceful in the container (Medium)

- **Where:** [server/src/main.rs:114-118](server/src/main.rs#L114-L118) and
  [deploy/entrypoint.sh](deploy/entrypoint.sh).
- **What happens:**
  - The server only listens for `ctrl_c` (SIGINT).
  - `exec` makes it PID 1 in the container. The kernel drops signals sent to
    a namespace's PID 1 unless it has a handler for them.
  - So `podman stop` sends SIGTERM, nothing happens, and after the stop
    timeout (10 s by default) podman sends SIGKILL.
  - Every deploy is therefore slow and cuts in-flight downloads hard.
- **Fix:** also handle SIGTERM with `tokio::signal::unix`. Bound the drain:
  axum waits for every open connection, and a 40 MB download on a slow link
  can take minutes, so add a timeout of about 20 s. Setting `StopSignal=SIGINT`
  in the Quadlet is a stopgap.

### D11: No access logs in production (Medium)

- **Where:** [server/src/routes.rs:42](server/src/routes.rs#L42) and
  [deploy/Caddyfile.snippet](deploy/Caddyfile.snippet).
- **What happens:** `TraceLayer::new_for_http()` logs requests and responses
  at DEBUG, and the deployment runs at `RUST_LOG=info`. Caddy's `log`
  directive is only on the internal `http://` block. Questions like "who
  downloaded X", "how many 404s since the last publish" or "which key is
  being hammered" cannot be answered.
- **Fix:** log one INFO line per response with method, path, status, bytes,
  duration, a 12-character token-hash prefix (never the token) and the
  user agent. Enable `log` on the HTTPS site in Caddy.

### D12: Smaller defects (Low)

- [installer/src/wizard.rs:729](installer/src/wizard.rs#L729) discards the
  catalogue error (`Err(_)`). The cause (TLS, 5xx, parse) never reaches the
  run log, which is the only diagnostic the user can send you.
- The cache index (`tree::CachedPackage`) stores neither `exclusive_group` nor
  `requires`, although [installer/CLAUDE.md:46](installer/CLAUDE.md#L46) says
  it does. Offline, Apply cannot enforce either. `size` and `updated_at` are
  missing too.
- [installer/src/wizard.rs:775](installer/src/wizard.rs#L775): one failed
  `/health` at scene 2 makes the whole run offline, even when `/packages`
  would have worked (a load balancer hiccup, or a 10 s timeout). Try
  `/packages` anyway, and use `/health` only for the status line.
- [server/src/routes.rs:128-131](server/src/routes.rs#L128-L131):
  `Content-Disposition` uses the full `name`, with `/`, spaces and perhaps
  `"`. The docs say it names the leaf. Use the leaf and RFC 6266
  `filename*=UTF-8''…`.
- [installer/src/zip.rs:172-213](installer/src/zip.rs#L172-L213): the
  central-directory loop stops quietly at the first unexpected signature and
  never checks the entry count from the EOCD record. A truncated directory
  yields a partial file list, which becomes a partial ledger. Fail when
  `entries.len() != count`.
- `shared::valid_name` accepts control characters. The RBR configuration step
  will derive `sounds=` from the name's segments, so a name containing `\r\n`
  could inject lines into `PaceNote.ini`. Reject bytes below 0x20 in
  `valid_name`. It is a one-line change in `shared`, and both sides benefit.
- `http::get` buffers the manifest and the MOTD with no size cap, so a
  hostile or misconfigured server can exhaust memory. Cap it at a few MB.

---

## 3. Install model pitfalls

I7 has not been built yet, so this is the cheapest moment to change the ledger
schema. Most of these reduce to one decision: **the ledger has to record more
than a list of paths.**

Suggested shape for a ledger entry:

```json
"rbr/public/codriver/polish/male/Kamil": {
  "sha256": "…", "version": "…", "exclusive_group": null,
  "state": "installed",                 // or "installing" (write-ahead, P6)
  "files": {
    "plugins/pacenote/pacenote.ini": {  // case-folded key (P5)
      "path": "Plugins/Pacenote/PaceNote.ini",   // as written
      "sha256": "…",                    // what we wrote (P2)
      "prior": "backup" | "identical" | "absent" // what was there before (P3, P4)
    }
  }
}
```

### P1: Ticks come from the cache, removals from the ledger (Critical)

- **What happens:** `Tree::build` ticks a leaf by default only when its
  payload is in the per-user cache. The I7 plan will uninstall whatever is
  "ledgered but not selected". The cache and the ledger are independent: the
  cache is per Windows user and can be cleared, while the ledger lives with
  the game folder. Every run where they disagree uninstalls things the user
  never touched. Cases where that happens:
  - a cleared cache, or a second Windows account;
  - the game folder copied to another PC;
  - a package the operator withdrew, or a grant that was revoked. If the
    package is not cached, it is not in the tree at all, so the user cannot
    even keep it. **A server-side change silently uninstalls content from
    users' games.**
- **Fix:**
  - Merge the ledger into the tree as its own state, "installed", and tick
    those leaves by default.
  - A ledgered package missing from both the server and the cache still
    appears as "installed (unavailable)", so the user can keep it or remove
    it.
  - Before phase 2, show a confirmation that lists what will be removed.
    Removal should never be a side effect the user cannot see.

### P2: Uninstall deletes files that are no longer ours (High)

- **What happens:** uninstall means "delete the ledgered files". Others write
  into game folders too: Steam's "verify integrity" and game updates, the RSF
  installer, plugin updates, other mod managers, and users editing ini files
  by hand. Once any of them replaces a file, uninstalling deletes their
  version.
- **Fix:** store the sha256 of what was written. On uninstall, delete a file
  only if it still matches, and otherwise leave it and log it. Restore needs
  the same comparison. The cost is a hash pass over the package files at
  uninstall, which is cheap next to extraction.

### P3: An identical-file skip turns pre-existing files into installer-owned ones (Critical)

- **What happens:**
  1. The user installed FilipekMod by hand.
  2. The installer installs the same package. The backup is skipped because
     the bytes match, and the files enter the ledger as the package's own.
  3. The user switches voice or uninstalls. The files are deleted, there is
     no backup, and the user's earlier setup is gone.
- The skip is the right call for disk space. Ownership just needs to remember
  what was there before.
- **Fix:** record `prior: "identical"` for each such file. On uninstall,
  restore its content instead of deleting it. The bytes are already in the
  cached payload (`<sha>.zip`, at a known entry path), so this costs a
  reference instead of a copy. The sweep has to treat payloads referenced this
  way as pinned.

### P4: Uninstall should put things back, not just delete (High)

- **What happens:** under the current plan, uninstall deletes files and
  Restore is a separate global action. Switching a voice off therefore removes
  a `PaceNote.ini` that existed before the installer ran, and the Pacenote
  plugin is left without config until the user finds Restore.
- **Fix:** handle each file according to its `prior` value:
  - `backup`: copy the backup back and delete the backup copy;
  - `identical`: restore from the cached payload;
  - `absent`: delete.

  Uninstalling then returns each file to its state before the installer
  touched it. The global Restore becomes a rarely needed "undo everything",
  and backups stop going stale (see P14).

### P5: Paths are case-insensitive on Windows (High)

- **What happens:** ledger keys are compared byte-for-byte. Packages from
  different authors and tools disagree on case (`Plugins/Pacenote` vs
  `plugins/pacenote`), and NTFS treats both as the same file. Ownership
  ("does another entry still list this file?") then gives the wrong answer,
  and an uninstall deletes a file that another package still needs.
- **Related cases:**
  - Windows strips trailing dots and spaces (`foo.` and `foo ` both become
    `foo`).
  - Reserved device names (`CON`, `NUL`, `AUX`, `COM1`, `LPT1`, `aux.ini`)
    open devices instead of files.
  - A zip can contain the same path twice.
- **Fix:**
  - Key the ledger on a case-folded form and keep the original spelling for
    display.
  - In `zip::safe_relative`, reject segments with a trailing dot or space and
    reserved device names.
  - Remove duplicate entries before extracting.

### P6: Partial installs poison the backup (Critical)

- **What happens:** uninstall runs before install, which is correct for the
  49 shared style paths. If extraction then fails (disk full, a file locked
  because the game is running, a CRC error halfway), two things go wrong:
  - the user ends up with neither the old package nor the new one;
  - the half-written files are not in the ledger, because the ledger is
    written after each package succeeds. On the next run they look
    user-owned, so they get **backed up as the pristine originals**. Restore
    would then put the broken files back.
- **Fix:**
  - **Pre-flight before touching anything:** inflate and CRC-check every
    entry of every package to be installed (or check that it was verified
    since it was cached), check free space for the installs plus the
    backups, probe that the folder is writable, and check that the game is
    not running.
  - **Write-ahead:** set the entry to `state: "installing"` with its planned
    files before extracting. A run that finds `installing` on start knows
    those files are its own, finishes or rolls back, and never backs them
    up.
  - Write each file to a temp name and rename it, so a crash never leaves a
    truncated game file.

### P7: Overlaps between packages from different categories (Medium)

- **What happens:** the rule "delete only if no other entry lists it" keeps
  the file but leaves the last writer's bytes in it. If a language pack and a
  voice pack both ship `PaceNote.ini`, removing the voice leaves the voice's
  version of the file, not the language pack's.
- **Fix:** at plan time, find every file that more than one selected package
  would write. Either refuse with a clear message (the operator should use
  `exclusive_group`), or keep an ownership stack and restore the previous
  owner's bytes from its cached payload. Refusing is simpler and makes these
  conflicts visible.

### P8: Two exclusivity mechanisms, one of them unimplemented (Medium)

- **What happens:** exclusivity comes from two places. Scene 5 enforces one
  pick per `CHOICE_CATEGORY`. `exclusive_group` is in the protocol, and
  DESIGN.md says "the UI radio-selects within a group", but nothing in
  `tree.rs` or `choices.rs` implements it. Two checked `pacenotes` extras in
  the same group would both install, over each other.
- **Fix:** decide on one rule, for example "a category in `CHOICE_CATEGORIES`
  is an implicit group; `exclusive_group` covers everything else". Then
  either implement the radio behaviour on scene 4, or have Download refuse
  more than one member of a group.

### P9: RBR's categories are hard-wired for every game (Medium)

- **What happens:** `CHOICE_CATEGORIES` (language, codriver, style) sits in
  `shared` and drives scene 5 for all four games. A BeamNG run would ask
  three pacenote questions with empty, disabled lists.
- **Second problem:** scene 4 requires a pick in **every** top-level category
  present. If the operator creates a new folder such as `extras/`, every user
  must tick something in it on their next run, so a content change changes
  the mandatory flow of the UI.
- **Fix:** move the choice categories into a per-game table (`gamedir.rs`, or
  a small per-game declaration in the manifest). Only choice categories
  should be mandatory; everything else should be optional. This also fits
  the rule that each game owns its own step.

### P10: Is the game root the right install root for every game? (Medium)

- BeamNG mods normally live under the user folder
  (`%LOCALAPPDATA%\BeamNG.drive\<version>\mods`), not next to
  `BeamNG.drive.exe`.
- Check each game before fixing the overlay root at "the folder that holds
  the marker". A declarative per-game install root still involves no
  scripts.

### P11: Folder permissions and UAC (Medium)

- **What happens:** the manifest asks for `asInvoker` and has `trustInfo`, so
  Windows does not virtualize writes. A game under `Program Files` (older RBR
  installs, some retail copies) fails with "access denied" in the middle of
  phase 2.
- **Fix:** at scene 3, try to create `.pacenotes\` and delete it again. If
  that fails, explain why and offer to restart elevated
  (`ShellExecuteW` with the `runas` verb), or stop with a clear message.

### P12: The game is running (Medium)

A running game holds its files open, and writes fail with a sharing violation
partway through the install. Check for the marker process, or try opening the
files exclusively, before phase 2. This belongs in the P6 pre-flight.

### P13: A package rename changes its identity (Medium)

- **What happens:** `game/directory/name` is the identity of a package. Fixing
  a typo in a folder (`Polsih` to `Polish`), moving a package between
  `public` and `premium`, or regrouping levels all create a new package. The
  consequences:
  - caches show the old one as "(local only)";
  - ledger entries are orphaned, and the user downloads everything again;
  - the RBR step's `sounds=` path changes.
- **Fix:** document it as a breaking operation in `deploy/README.md`.
  Optionally, add a sidecar key `replaces = ["public/codriver/polsih/male/Kamil"]`
  so the installer can carry ledger and cache entries across a rename.

### P14: The restore design is not settled (Medium)

- [installer/DESIGN.md:74](installer/DESIGN.md#L74) describes "a list of
  selectable previous backup packages", but the model has one first-touch
  backup tree.
- To be truly "pristine", a restore also has to delete the files packages
  added and clear the ledger. Both depend on the ledger existing.
- First-touch backups never expire. A backup from 2026 restored after a 2027
  plugin update rolls the plugin back. Show backup dates, and with P4 most
  backups are consumed on uninstall anyway.

### P15: Memory use (Low)

A zip is read whole, and each entry is inflated whole. That is fine at 44 MB,
but voice packs grow. Seeking per entry in the cached file keeps the peak at
the size of one entry.

---

## 4. Server, protocol and publishing

### S1: No protocol version and no client version (High)

- **What happens:** `Manifest` has no schema version, the user agent is
  hard-coded as `Pacenotes-Installer/0.1`, and there is no minimum supported
  client and no update channel. Old `.exe` files live on for years, so the
  next protocol change will break them without any message.
- **Fix:**
  - Add `schema: u32`, `generated_at`, a `generation` hash and
    `min_installer: Option<String>` to `Manifest`.
  - Take the user agent from `CARGO_PKG_VERSION`.
  - Have the installer show "please download the new installer" when it is
    too old.
  - The server logs the user agent (D11).

### S2: The server is trusted for everything, including code (Medium; High for multi-host)

- **What happens:** the sha256 that protects a download comes from the same
  server as the zip. Overlay zips can write DLLs into the game folder, so a
  compromised server or mirror gets code execution inside the game process.
  The "no scripts in packages" rule protects the installer, not the user.
- **Fix:** sign the manifest with ed25519 and embed the public key in the
  installer. Mirrors, CDNs and object storage then cannot tamper with
  packages. Measure the verifier against the 500 kB budget first; a
  compact ed25519 implementation is small.

### S3: Folder names are untranslated UI labels (Low)

Level labels such as `codriver`, `male` and `polish` show as raw lowercase
folder names in both the English and the Polish UI. An optional per-level
label map (for example `_labels.toml` in a folder, merged into the manifest)
would keep the layout as the single source of truth and allow localized
labels.

### S4: Publishing is not atomic (High)

- **What happens:**
  - `cp zip …` writes in place. A download that starts during the copy
    streams a partial file. Content-Length comes from the file, so the
    client gets a short body with a matching length, fails the sha check and
    reports "arrived corrupted".
  - Replacing a zip in place before `manifest gen` runs makes **every** client
    fail verification until it does.
  - [server/src/routes.rs:115](server/src/routes.rs#L115) deliberately trusts
    the file over the manifest, which hides this.
- **Fix, best option:** content-addressed blobs. Store
  `data/<game>/blobs/<sha256>.zip`, have the manifest point to the sha, and
  keep the folder tree as the authoring view, which `manifest gen` hashes and
  copies into blobs. Published bytes then never change.
- **Fix, minimal option:** document "copy to `*.tmp`, then rename". In the
  handler, return 503 and log an error when `file size != package.size`.

### S5: `manifest gen` problems (Medium)

- It hashes every zip on every start. Reuse the previous sha when the path,
  size and mtime are unchanged.
- It writes `manifest.json` with `std::fs::write`, which is not atomic. Write
  to a temp file and rename it.
- A single broken sidecar fails the whole run. The entrypoint swallows the
  failure (`|| echo`), so the catalogue silently stays on the previous
  manifest and nothing reports it. Skip the broken package with an error,
  exit non-zero, and expose the state on `/ready` (S6).
- Add `--check`/`--dry-run`: print the diff against the current manifest and
  the warnings, without writing anything. Add `--strict`, which fails on
  warnings, for publish pipelines.

### S6: `/health` returns "ok" even with nothing to serve (Medium)

Add `/ready`, reporting whether the manifest loaded, the package count, the
generation or `generated_at`, and whether the DB is reachable. Orchestrators,
uptime checks and you can then tell "up" from "serving an empty catalogue".
The Quadlet `HealthCmd` acts on nothing today, because no `HealthOnFailure=`
is set.

### S7: SQLite settings and migrations (Medium)

- There is no `journal_mode=WAL` and no `busy_timeout`. A
  `podman exec … token mint` while the server is reading can return
  `SQLITE_BUSY`, which surfaces as a 500 to a user.
- `PRAGMA foreign_keys` is off by default, so `ON DELETE CASCADE` does
  nothing. Nothing hard-deletes tokens yet, so this is latent.
- Migrations are re-applied on every start because the statements are
  idempotent. The first `ALTER TABLE` will break that, and with several
  replicas every instance runs migrations concurrently. Track the version
  with `PRAGMA user_version` or a `schema_migrations` table, and make one
  process (the CLI) responsible for migrating.
- One shared `libsql::Connection` serializes every request. For the remote
  backend, check stream reuse under concurrency. Keeping the `Database`
  handle and opening a connection per request is the safer pattern.

### S8: Operator visibility into credentials (Low)

There is no `last_used_at` and no per-key download count, so key sharing on
paid content cannot be noticed. `token list` could show both. Add a
`token rotate` (mint, copy grants, revoke the old key) to make rotation one
command.

### S9: Backups (Medium)

- Copying `auth.db` while the server runs can produce an inconsistent copy.
  Use `sqlite3 auth.db ".backup …"` or `VACUUM INTO`, or add a
  `token export` command.
- [deploy/README.md:235](deploy/README.md#L235) says packages and
  `manifest.json` are reproducible. Packages are operator content and need a
  backup or another authoritative copy, and the manifest is not reproducible
  either (D5).

---

## 5. Running on several servers or in the cloud

What ties the current design to a single host:

| Coupling | Where | Breaks when |
|---|---|---|
| One mutable volume holds `auth.db`, packages and `manifest.json` | Quadlet `Volume=` | two instances share it: SQLite over a network filesystem is unsafe |
| Every instance runs `manifest gen` at startup | `entrypoint.sh` | instances write one file at the same time, or each produces its own manifest (D5) |
| Reload is triggered by the mtime of `manifest.json`, checked by a blocking `stat` on every request | `store.rs:78-114` | object-storage FUSE mounts and some network filesystems report mtime poorly; `stat` is slow on them |
| Packages are served by `tokio::fs::File::open(path)` | `routes.rs:104` | packages live in object storage |
| No cache headers on any response | `routes.rs` | a CDN goes in front (C2) |
| The MOTD is read from disk on every request | `store.rs:161` | there is no shared disk |

A target that keeps the current simplicity:

1. **Stateless instances.** Auth moves to remote libSQL or Turso. The code
   already supports it, since `--db-url` is the only change. Embedded
   replicas would give local-latency reads, but they need libsql's
   `replication` feature, which was dropped to fit a build on the 3 GB host.
   Building in CI (O1) removes that constraint.
2. **Publishing becomes a job, not a side effect of startup.** `manifest gen`
   runs once, wherever the content is authored (CI or an admin box). It
   uploads the content-addressed blobs (S4) and the manifest with its
   `generation` id (S1). Instances load the manifest from storage on a timer
   or on SIGHUP and swap it in with `Arc` (no per-request clone).
3. **Downloads.** Public blobs are immutable, so they can come straight from a
   CDN with `Cache-Control: public, max-age=31536000, immutable`. Protected
   blobs are authorized by the server, which then redirects to a short-lived
   signed URL.
4. **A server-side `PackageSource` abstraction** (local filesystem, object
   store, redirect), so `routes.rs` stays the same. This belongs on the server
   only; `shared` does not change.

### C1: The installer must handle redirects itself before v1 ships (High)

- **What happens:** WinHTTP follows redirects automatically. As far as I
  know, it re-sends headers added with `WinHttpAddRequestHeaders`, including
  `Authorization`. **Verify this with a test.** If so, redirecting to a signed
  URL would:
  - send the user's access key to the storage provider;
  - fail anyway, because S3 and compatible stores reject a request that
    carries both a pre-signed query and an `Authorization` header (400).
- **Why now:** installers already in the wild cannot be patched. Once one
  release ships with automatic redirects, the server can never start
  redirecting.
- **Fix:** disable automatic redirects (`WINHTTP_OPTION_DISABLE_FEATURE` with
  `WINHTTP_DISABLE_REDIRECTS`), follow at most N hops by hand, allow
  `https` only, and drop `Authorization` whenever the host changes. This is a
  few dozen lines in `http.rs`.

### C2: Cache headers, before any CDN goes in front (High)

- `/packages` varies by credential. Send `Cache-Control: private, no-store`,
  or at least `Vary: Authorization`. Otherwise a shared cache can serve one
  key holder's listing to everyone.
- Protected `/download` responses: `Cache-Control: private`. The URL is the
  same for every caller and authorization is only a header, so a
  "cache everything" rule would hand premium zips to anonymous users.
- Public downloads: `ETag: "<sha256>"`.
- Add `Accept-Ranges` and Range support, plus resume in the installer.
  Without them, a large voice pack that drops at 90 % restarts from zero.

### C3: Hostname and fallback (Medium)

- A release build compiles in `PACENOTES_DEFAULT_SERVER` from `installer/.env`
  ([installer/build.rs](installer/build.rs)), so a shipped binary carries one
  hostname for good. Moving hosts only works through DNS, so keep that name under the project's control. Never
  ship a provider hostname (`*.fly.dev`, `*.azurewebsites.net` and the like).
- Optional: a second hostname as a fallback, tried when the first is
  unreachable.

### C4: Abuse and cost (Medium)

- Cloud egress is billed, and `/download` has no rate limit. Add per-IP
  limits at Caddy, the CDN or the server.
- Reject malformed bearer tokens before the DB lookup (D9). On remote Turso,
  every lookup is billed.
- Logs need to be structured (a JSON option) so a log aggregator can parse
  them.

---

## 6. Deployment and CI

### O1: Every start of the Quadlet runs a build (High)

- **Where:** `ExecStartPre` in
  [deploy/pacenotes.container](deploy/pacenotes.container) runs
  `git clone` and `podman build` from the tip of `BRANCH_TO_BUILD`. With
  `Restart=on-failure`, that includes recovery after a crash. Consequences:
  - Recovering from a crash takes minutes, because it rebuilds first.
  - Whatever is on `master` at that moment is deployed, without a decision.
  - If the git host, crates.io or Docker Hub is down, the server stays down
    after a reboot.
  - The README's publishing path ("drop the zip in, restart") **also deploys
    new code**.
  - Builds run on the 3 GB production host, next to the services it runs,
    and have already caused OOM kills.
  - Every image is tagged `latest`, so there is nothing to roll back to.
- **Fix (M7 already lists part of this):** CI builds the image, tags it with
  the git sha and pushes it to a registry. The Quadlet pins that tag, so a
  restart never builds. Rolling back means pinning the previous tag. Publish
  content with `manifest gen` alone (S5).

### O2: Smaller deployment and CI issues (Low to Medium)

- [deploy/pacenotes.container:45](deploy/pacenotes.container#L45)
  hard-codes `/home/user/.config/...` for `EnvironmentFile`, while the README
  installs to `~/.config/...`.
- [deploy/Caddyfile.snippet:18](deploy/Caddyfile.snippet#L18) binds the
  plain-HTTP site to `0.0.0.0`. If the Caddy container publishes port 80 (it
  usually does, for ACME), bearer tokens can travel unencrypted from outside.
  Bind it to the container network or loopback only, or remove it.
- Large data tree: the `:Z` volume option relabels the whole tree on every
  start, which gets slow as packages accumulate.
- `ci.yml` runs on `master`, but the repository's main branch is reported as
  `main`. Check that pull requests into the default branch actually trigger
  CI.
- The toolchain floats (`dtolnay/rust-toolchain@stable`,
  `rust:1-slim-bookworm`), and CI uses `-D warnings` and a size gate. A new
  rustc or clippy release can fail CI, or move the binary size, without any
  commit. Pin the toolchain in `rust-toolchain.toml` and pin base images by
  digest. Actions are pinned by tag, not by sha.
- The release `.exe` is unsigned. A small binary that downloads files and
  writes them into a game folder is a typical SmartScreen and antivirus
  heuristic hit. Sign it with Authenticode in the CD workflow, and publish
  its SHA-256 alongside the release.
- The server has no versioned artifact, release or tag (an M7 item).

---

## 7. Support and observability (installer)

- **Run log** ([installer/src/runlog.rs](installer/src/runlog.rs)):
  - Lines have no timestamps, and the log has no header with the installer
    version, server URL, manifest `generation`, Windows version and game
    folder. Those are the first things support asks for.
  - The log is overwritten on every run, so a user who reopens the
    installer to "see what happened" erases the failed run. Keep the last
    three (`install.log`, `install.1.log`, and so on).
- **Correlation:** send an `X-Request-Id`, or at least a versioned user
  agent, and log it on both sides so a user's log can be matched against
  server logs.
- **Storing the secret:** I8 plans a plain file. Use DPAPI
  (`CryptProtectData` from `crypt32`; no crate needed) so the key is not
  readable by other users or by malware scanning text files.
- **Error recovery:** a single failed package aborts the whole run
  ([installer/src/worker.rs:298](installer/src/worker.rs#L298)). Retry
  transient errors with backoff, carry on with the other packages, and list
  the failures on scene 7.
- **One TLS handshake per request:** `get_streaming` opens a new WinHTTP
  session each time. Reusing one session for the whole run saves a handshake
  per package. This matters little now but adds up with dozens of packages.
- **Free space:** I9's check should count downloads, plus extracted size,
  plus backups.

---

## 8. Documentation that disagrees with the code

| Document | Says | Actually |
|---|---|---|
| [CLAUDE.md:122](CLAUDE.md#L122) | "the status never reveals what exists" | it does: 401 vs 404 (D3) |
| [installer/DESIGN.md:53](installer/DESIGN.md#L53) | `/packages` returns 401 for an invalid secret | it never does (D2) |
| [installer/DESIGN.md:74](installer/DESIGN.md#L74) | restore offers "a list of previous backup packages" | the model has one backup tree (P14) |
| [installer/CLAUDE.md:46](installer/CLAUDE.md#L46) | `cache.json` stores size, updated_at, exclusive_group, requires | it stores none of them (D12) |
| [server/CLAUDE.md:103](server/CLAUDE.md#L103) | `Content-Disposition` names the leaf | it uses the full name (D12) |
| [deploy/README.md:235](deploy/README.md#L235) | packages and `manifest.json` are reproducible | neither is (D5, S9) |
| [installer/src/cache.rs:116](installer/src/cache.rs#L116) | "Windows rename fails onto an existing file" | not true for `std::fs::rename` (D6) |
| [installer/src/api.rs:102](installer/src/api.rs#L102) | ".part names stop concurrent runs sharing a file" | they share it when both download the same package (D7) |
| [installer/tests/tree.rs:298](installer/tests/tree.rs#L298) | "so it must refetch" | the test asserts the opposite (D1) |
| [server/TASKS.md:65](server/TASKS.md#L65) | sidecar categories `sounds/configs/languages/pacenotes` | categories come from paths now; a `category` key is rejected |
| [server/CLAUDE.md:76](server/CLAUDE.md#L76) | "Security invariantsr" | typo |
| [deploy/pacenotes.container:45](deploy/pacenotes.container#L45) | `/home/user/.config/...` | the README uses `~/.config/...` |

---

## 9. What is solid (keep it)

- Names and directories are lookup keys, never path fragments, and are checked
  on both sides (`valid_id`, `valid_name`). Traversal is ruled out by
  construction.
- The zip-slip guard (`safe_relative`) is plain string logic covered by
  string tests. CRC checks at extraction catch a cache that has rotted on
  disk.
- A download goes to a temp file, is hashed while streaming, and is promoted
  only on a match. Nothing unverified enters the cache.
- Raw tokens are never stored or logged. `is_live` is defined once.
  `token list` shows a short hash.
- `deny_unknown_fields` on sidecars, with a hard failure.
- Each game owns its configuration step, with no shared abstraction. RBR's
  step edits `PaceNote.ini` instead of regenerating it.
- The test discipline: real fixtures, and assertions against the source of
  truth (the `.rc` strings).

---

## 10. Task list

Every finding in sections 2–8 appears below at least once, under the period
by which it should be handled. The IDs point back to the findings, where the
reasoning is. A fourth period, **Backlog**, holds the low-priority items that
have no deadline.

Protocol changes follow the root `CLAUDE.md` rule: change `shared/` first,
then both sides in the same PR.

### Period 1: before writing I7

**Status: handled on 2026-10-25** (commits `396e6f4` and the one that
follows it). The decisions are recorded in `installer/CLAUDE.md`,
`installer/DESIGN.md` and the I7 checklist in `installer/TASKS.md`; the
code fixes are done and tested; the "build into I7" items are requirements
on I7 rather than work that could be finished before it exists, so they are
reproduced in that checklist and ticked here as *specified*, not as built.

#### Decisions (recorded in `installer/CLAUDE.md`)

- [x] **P2–P6** Ledger schema: per-file `sha256`, `prior`
      (`backup` | `identical` | `absent`), case-folded keys with the original
      spelling kept, and `state: "installing"` for write-ahead.
      **Adopted as proposed.**
- [x] **P7** Overlapping files: **last writer wins, logged, and reported to
      the server**. Neither of the two options offered — a user must never
      be blocked by a catalogue mistake they cannot fix, and a line in a
      log nobody reads does not reach the one person who can fix it. The
      installer therefore sends a conflict report (`POST /report/conflict`;
      `server/TASKS.md` S10). The known cost is accepted and written down:
      after uninstalling the winner, the file survives holding the winner's
      bytes.
- [x] **P8** Exclusivity: **engine only**. A choice category is an implicit
      group (scene 5 asks for one pick); `exclusive_group` is superseded in
      `plan()`. No gate on scene 4. Two members ticked in one run both
      install, the later wins — which is exactly the P7 case, and is
      reported the same way.
- [x] **P9** Choice categories move to a per-game list in `gamedir.rs`,
      each with a localized label AND explanation. Not the manifest: that
      text is localized UI, and the server cannot know the installer's
      languages. Only choice categories are mandatory on scene 4.
      **Consequence accepted and specified:** the manifest needs a version,
      and an installer older than the catalogue blocks DOWNLOADING only —
      reconfiguring and uninstalling keep working (`server/TASKS.md` S11).
- [x] **P10** Install root becomes a per-game property alongside the
      categories. RBR = the game folder, as today. The other three stay
      undecided until that game has packages, and deciding is part of
      adding the game; the BeamNG note (mods under `%LOCALAPPDATA%`) is
      recorded there so it is not rediscovered.
- [x] **P14** Restore: **dated restore points**, chosen from a list, each
      pairing the replaced files with the ledger as it was. Restoring also
      REMOVES what was installed after that point — copying files back
      alone leaves a broken package's added files behind, which is the bug
      the legacy restore had. The scene previews what the restore will do
      before it does it.

#### Shared

- [x] **D4** `Manifest::get(game, &PackageRef)`, used in
      `validate_requires`; refs resolve within the package's own game, and
      a same-named package in another game no longer shadows the real one.
- [x] **D12** `valid_name` rejects control characters (below 0x20, and
      0x7f).

#### Installer: fix in the existing code

- [x] **D1** `has_payload()` / `needs_download()`, with three tests: an
      update is ticked AND downloaded, a current leaf is not refetched, a
      local-only leaf is never queued.
- [x] **D6** Rename straight over `cache.json`; `version` in `Index`;
      `open` tells Missing, Unreadable and Loaded apart. The old comment
      was measured and was wrong — `std::fs::rename` replaces on Windows.
- [x] **D6** Sweep rules: `orphans()` returns nothing unless the index was
      genuinely read, so the rule cannot be forgotten by the I9 caller.
      "Only orphans older than N days" is recorded in the `orphans()` doc
      comment for I9, which is where the age policy belongs.
- [x] **D7** Named mutex at startup with an "already running" modal.
      Verified with two live processes. The UI harness refuses to launch
      while another copy runs, so this never masquerades as a missing scene.
- [x] **D12** The cache index stores `exclusive_group`, `requires`, `size`
      and `updated_at`.
- [x] **D12** Zip parsing fails when the entry count disagrees with the
      EOCD record.

#### Installer: build into I7 (specified, not yet built)

Each of these is now a line in `installer/TASKS.md` I7 with its reasoning.

- [x] **P1** Ledger merged into the tree as an "installed" state, ticked by
      default; ledgered packages missing from server and cache shown as
      "installed (unavailable)".
- [x] **P1** Confirm the removal list before phase 2.
- [x] **P2** Uninstall deletes a file only when its hash still matches.
- [x] **P3** `prior: identical` restores from the cached payload; those
      payloads are pinned against the sweep.
- [x] **P4** Uninstall handles each file by its `prior`.
- [x] **P5** Case-folded ledger keys; `safe_relative` rejects trailing dots
      and spaces and reserved device names; duplicate zip entries removed.
- [x] **P6** Pre-flight before any uninstall (CRC, free space, writability,
      game not running — **P12**).
- [x] **P6** Write-ahead `installing` state with recovery.
- [x] **P6** Temp name then rename, per file.
- [x] **P7** The overlap rule above, plus the report.
- [x] Table tests for all of it, listed in the I7 checklist.

#### CI

- [x] **O2** Checked: both remotes' HEAD is `master`, so `ci.yml` triggers
      correctly. The `main` in the session header was a tool's guess, not
      the repository's default branch. No change needed.

#### Documentation, fix now

- [x] `installer/tests/tree.rs`: the comment that contradicted its
      assertion is gone; the test now asserts the download (D1).
- [x] `installer/src/cache.rs`: the rename comment (D6).
- [x] `installer/src/api.rs`: the `.part` comment (D7).
- [x] `installer/CLAUDE.md`: the cache index fields — the code now stores
      what the document claimed (D12).
- [x] `installer/DESIGN.md`: the restore scene, rewritten around restore
      points (P14).
- [x] `server/TASKS.md`: stale sidecar categories.
- [x] `server/CLAUDE.md`: typo "invariantsr".

### Period 2: before the first public release

**Status on 2026-10-27.** The SERVER side of this period is finished, and
so is the protocol except for `generated_at`/`generation`, which Period 3's
`/ready` wants anyway. The restore scene (P14) closed the last functional
gap in the installer.

What is left is a handful of small installer items and the whole of deploy
and CI, which nobody has started. Both lists moved out of this document —
see "Installer, deploy and CI" below.

#### Protocol

- [ ] **S1** Add `schema`, `generated_at`, `generation` and `min_installer`
      to `Manifest`. The installer asks for an update when it is too old.
      *Superseded in part on 2026-10-25: the maintainer chose a per-game
      install schema checked by raw-file SHA instead of a manifest version
      (`server/TASKS.md` S11; server side done). `generated_at` /
      `generation` remain open for Period 3's `/ready`.*
- [x] **D2** `GET /whoami` (or a response header) returning
      `{ valid, expires_at }`. Scene 2 uses it, and warns when a key is about
      to expire. *Both halves done: server 2026-10-25 (`/whoami`: 200
      `{expires_at}` / 401), installer 2026-10-25 — scene 2 validates there
      now, and a key within 14 days of expiry says so with the date.*
- [x] **C2** `Cache-Control: private, no-store` and `Vary: Authorization` on
      `/packages`; `Cache-Control: private` on protected downloads;
      `ETag: "<sha256>"` on public ones.

#### Server

- [x] **D3** Return 404 for a non-public directory to any caller without a
      grant for it (or 401 whether or not the package exists). Update the
      matrix in `server/tests/api.rs`.
- [x] **D8** Fail closed when a token row cannot be decoded.
- [x] **D9** Skip the token lookup for public packages, and fall back to the
      public listing on a DB error.
- [x] **D9** Reject tokens that are not `pcn_` plus 43 characters before
      querying (also **C4**).
- [x] **D10** Handle SIGTERM, and bound the drain to about 20 s. Stopgap:
      `StopSignal=SIGINT` in the Quadlet.
- [x] **D11** One INFO line per response: method, path, status, bytes,
      duration, token-hash prefix, user agent.
- [x] **D12** `Content-Disposition` names the leaf, with RFC 6266
      `filename*`.
- [x] **S4** Minimal fix: return 503 and log an error when the file size
      differs from the manifest size.
- [x] **S7** `journal_mode=WAL` and `busy_timeout`.

#### Publishing (`manifest gen`)

- [x] **D5** The manifest becomes a pure function of zips plus sidecars, with
      `version` required or derived.
- [x] **D5** A one-off `manifest migrate-to-sidecars` command for the values
      that are only in the current manifest.
- [x] **S5** Write `manifest.json` atomically (temp file, then rename).
- [x] **S5** A broken sidecar skips its package and makes the run exit
      non-zero. The entrypoint reports the failure visibly instead of only
      echoing it.

#### Installer, deploy and CI — moved to the task lists

These were a second copy of a todo list, kept in step by hand, and on
2026-10-27 that drifted far enough to be worth ending. The items now live
where work is picked up; the FINDINGS they came from stay above, with the
reasoning and the severity:

- installer items (C1, S1's user agent, the two D12s, P11, the §7 log
  header, retries and DPAPI) → `installer/TASKS.md`, "Pre-release
  hardening";
- deploy, CI and supply chain (O1, O2, C3, S9, and the two
  `deploy/README.md` notes) → `TASKS.md`, M7.

Done here since the review was written, for the record:

- [x] **D12** The catalogue error is written into the run log.
- [x] **S1** The user agent is `Pacenotes-Installer/<version>`, taken from
      the build, so the server's access log (D11) names the build behind
      every request.
- [x] **D12** Scene 4 asks for the catalogue even when `/health` failed;
      `/health` decides only scene 2's status line.
- [x] **D12** `http::get` refuses a body over 8 MiB, counted as it arrives.
- [x] **P11** Scene 3 probes the folder when a route is taken and, when this
      account cannot write it, offers to restart as administrator, resuming
      on scene 3 with the language, folder and key.
- [x] **§7** The run log opens with the build, the start time, the server
      and the Windows build, and times every line in UTC — in the shape the
      server's log uses, so the two can be laid side by side.
- [x] **P14** The restore scene, with two rules the design had not named:
      a package whose payload is gone is left installed rather than
      removed, and the ledger afterwards keeps the snapshot's per-file
      provenance.
- [x] **§7** One log file per run, in the game folder, newest five kept —
      so re-opening the installer no longer erases the failed run.
- [x] A panic leaves a report naming its file and line. Not in the review:
      added after a crash left nothing at all.
- [x] [CLAUDE.md](CLAUDE.md): the "status never reveals what exists" claim
      (D3) — now true, and the text describes how.
- [x] [installer/DESIGN.md](installer/DESIGN.md): how the secret is
      validated (D2) — scene 2 documents `/whoami`, why it is not
      `/packages`, and what "could not ask" does.
- [x] [server/CLAUDE.md](server/CLAUDE.md): `Content-Disposition` (D12).
- [x] [deploy/README.md](deploy/README.md): what is reproducible (D5, S9).

### Period 3: before more than one server

#### Storage and publishing

- [ ] **S4** Content-addressed blobs (`data/<game>/blobs/<sha256>.zip`), with
      the folder tree kept as the authoring view.
- [ ] **§5** Publishing as a job: `manifest gen` runs once and uploads the
      blobs and the manifest with its `generation` id.
- [ ] **§5** Instances load the manifest from storage on a timer or on
      SIGHUP, and swap an `Arc`: no clone and no `stat` per request.
- [ ] **§5** A server-side `PackageSource` (local filesystem, object store,
      redirect).
- [ ] **§5** Serve the MOTD from the same storage as the manifest.
- [ ] **S5** Reuse the previous sha when path, size and mtime are unchanged.
- [ ] **S5** `manifest gen --check` (a dry run with a diff) and `--strict`
      (fail on warnings).

#### Downloads

- [ ] **§5** Public blobs from a CDN with immutable cache headers. Protected
      blobs authorized by the server, then redirected to a short-lived signed
      URL. This needs **C1** to have shipped.
- [ ] **C4** Per-IP rate limits on `/download`.

#### Auth and database

- [ ] **§5** Turso, or embedded replicas (re-adding libsql's `replication`
      becomes affordable once **O1** builds in CI).
- [ ] **S7** Track the migration version (`user_version` or
      `schema_migrations`). The CLI migrates, the server only checks.
- [ ] **S7** Keep a `Database` handle and open a connection per request.
      Check that remote streams can be reused under concurrency.
- [ ] **S7** `PRAGMA foreign_keys = ON`.

#### Operations

- [ ] **S6** `/ready`: manifest loaded, package count, generation, DB
      reachable. Wire it to `HealthOnFailure=` or the orchestrator's probes.
- [ ] **C4** An option for structured JSON logs.

#### Security

- [ ] **S2** An ed25519-signed manifest, with the public key embedded in the
      installer. Measure the verifier against the 500 kB budget first.

### Backlog (low priority, no deadline)

- [ ] **C2** Range requests and `Accept-Ranges`, with resume in the
      installer.
- [ ] **C3** A fallback hostname.
- [ ] **P13** An optional sidecar key `replaces = [...]` that carries ledger
      and cache entries across a rename.
- [ ] **P15** Seek per entry instead of reading the whole zip.
- [ ] **S3** A per-level label map, for localized tree labels.
- [ ] **S8** `last_used_at` and a per-key download count in `token list`,
      and a `token rotate` command.
- [ ] **O2** Avoid the `:Z` relabel cost on a large data tree.
- [ ] **§7** Reuse one WinHTTP session for a whole run.
