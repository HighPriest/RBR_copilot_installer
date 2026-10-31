# Installer requirements

What installer work has to satisfy. The RULES that constrain the code are
in `installer/CLAUDE.md`, the BEHAVIOUR screen by screen is in
`installer/DESIGN.md`, and how any task is verified is in the root
`requirements.md`.

Every task here also ends with: it builds on the Windows MSVC target,
`tests/Test-BinarySize.ps1` passes, and `cargo test` is green for the
logic modules on any host.

## The size budget

**600 kB.** It was 500 kB until the install engine landed, and it moved
once, deliberately. The rules it produced did not move: no tokio, no
reqwest, no rustls, no GUI framework, and `std` over a dependency.

Measured on the release build for `x86_64-pc-windows-msvc`:

| After | Bytes | |
| --- | --- | --- |
| nothing (baseline) | 107,008 | 104.5 kB |
| I1 — icon + VERSIONINFO | 193,024 | 188.5 kB |
| I3 — WinHTTP client | 237,056 | 231.5 kB |
| I2b part 1 — wizard + worker | 275,456 | 269 kB |
| I2b part 2 — tree model + scene 4 | 337,408 | 329.5 kB |
| I5 — sha256 + cache + verified download | 402,432 | 393 kB |
| I2b — scene 5 + the bigger page | 412,160 | 402.5 kB |
| I2b — scenes 6 and 7 + the run log | 427,520 | 417.5 kB |
| I7b — the ledger reachable from main | 452,608 | 442 kB |
| I7f — the engine wired in (gate raised to 600 kB) | 553,984 | 541 kB |
| restore scene + the recut icon | 590,336 | 576.5 kB |

Two levers are held in reserve, in this order:

1. **The icon.** It was a single 256×256 frame costing 85 kB — more than
   serde and the whole engine together. It is now 16/32/48 as BMP plus 128
   as PNG, about 45 kB, built by `res/Build-Icon.ps1` from
   `res/rbr-source.png`. Dropping the 128 frame would buy ~31 kB more.
2. **serde_json**, about 48 kB with serde_core. The installer reads three
   documents — the manifest, `cache.json`, `installed.json` — and a
   hand-rolled parser over those is the planned fallback.

`docs/bloat.txt` holds the last measurement. `build.rs` watches
`../rbr.ico`, so changing the icon does rebuild.

## Specs for the open work

### C1 — redirects

**ANSWERED, 2026-10-30, measured: WinHTTP re-sends `Authorization` to the
new origin.** Two local listeners on two ports; the first answered 302 to
the second. Port B received `Bearer <token>` in full, and the client
reported `RESULT: OK` with B's body — it never saw the 3xx and had no idea
the token had gone somewhere else.

So the leak is real, not theoretical. A redirect today sends the user's
access key to whatever host a 302 names: a misconfigured CDN, a hijacked
DNS record, or an operator who moved downloads without thinking about the
header. Nothing in the installer would notice or log it.

The fix, which was always going to be the shape regardless of the answer:

- redirects disabled (`WINHTTP_OPTION_REDIRECT_POLICY`), followed by hand;
- a small hop limit;
- `https` only — never a downgrade;
- `Authorization` dropped the moment the host changes.

A bearer token must not reach a host the operator merely pointed at.

`tests/Test-Redirects.ps1` pins three of the four: a cross-origin redirect
is followed WITHOUT the token, a same-origin one keeps it, and a loop ends
by itself. The https→http refusal is NOT covered — an `HttpListener` on
loopback cannot easily serve real TLS, and `http.rs` lives in the bin, out
of reach of `tests/`. It is a two-line check (`target.secure &&
!next.secure`) sitting directly above the hop that would otherwise happen,
and it is the one piece of this policy resting on review rather than on a
test. A test for it needs either a loopback TLS certificate or `http.rs`
moved into the lib.

### I10 — the version comes from the tag

**Decided 2026-10-31:** a release is a pushed tag shaped `v/X.Y.Z`, and
that tag is the only place a version is written. Nothing is bumped by hand.

- `build.rs` owns the rule, so it lives in one place: it reads
  `PACENOTES_RELEASE_TAG` (CI sets it to the pushed tag), accepts exactly
  `v/` plus three decimal numbers, each 0..=65535 because VERSIONINFO
  stores them as 16-bit words, and FAILS THE BUILD on anything else. A
  release built from a mistyped tag must not exist.
- Without the variable the build is a development one, `0.0.0-dev`
  (VERSIONINFO `0,0,0,0`), so a dev build is recognisable in the server's
  access log and in the file properties.
- One value reaches every place a version shows: the user agent, the
  conflict report's `installer` field, and all four VERSIONINFO fields,
  which come from a header `build.rs` writes into `OUT_DIR` for the `.rc`
  to include.
- `Cargo.toml`'s `version` is fixed at `0.0.0` with a comment saying the
  tag decides. Leaving a real-looking number there would invite someone to
  bump it.
- CD releases when a `v/*` tag is pushed, and names the GitHub release
  after that tag. The release on merge to `master` and the manual one both
  READ `Cargo.toml` and CREATE the tag; with nothing to read they go.
- Verified by building, not by reading files: the test builds from a real
  tag and reads the version back out of the binary's file properties and
  its user agent string, and checks that malformed tags fail the build.

### §7 — the run log's header

Support asks for three things first: when, which build, and which server.
The log has none of them. A header at the top of each file, plus a
timestamp per line.

### D12 — `/packages` without `/health`

One dead endpoint must not declare the whole run offline. The catalogue is
the request that actually matters; `/health` is a convenience.

### D12 — a cap on buffered bodies

`http::get` reads a whole body into memory with no limit. A manifest is
small, but that is the server's promise rather than ours.

### P11 — the writability probe

A game under `C:\Program Files` currently fails partway through the
install with an access error. Probe at the folder page and say so, with an
offer to restart elevated: a sentence before anything is written beats a
half-finished install.

**Decided 2026-10-31:**

- The probe runs when a route button is pressed — the moment the user
  commits to the folder — not while they type. It creates `.pacenotes\`
  and a file in it, and removes whatever it created: a folder the user
  then abandons keeps no trace.
- Not writable → explain, and offer to restart as administrator (UAC, via
  `ShellExecuteW` with `runas`). Declining, or cancelling the UAC prompt,
  leaves the user on scene 3 to choose another folder.
- Already elevated and still not writable (read-only media, a deny ACL) →
  explain only; offering elevation again would loop.
- The elevated instance opens on scene 3 with the LANGUAGE, the FOLDER and
  the ACCESS KEY carried over, so nothing is asked twice. The key travels
  on the elevated process's command line, readable by other programs
  running as this user — accepted by the maintainer for the seamless
  restart.
- The restarted instance waits for this one to exit rather than reporting
  "already running".
- Files an elevated run installs belong to Administrators, so later runs
  on that folder need elevation too. Inherent to the choice; the probe
  will say so each time.

### §7 — retrying a transient download

A single failed package ends the run. Retry with backoff, carry on with
the others, and list what failed on scene 7.

Settled while building it (2026-10-31):

- **Transient** means no response at all (a connection or timeout
  failure), 408, 429 or 5xx, and a payload that arrived corrupted (its
  sha256 did not match). 401 and 404 will not change on a retry, and a
  local disk error is not the network's fault, so those fail at once.
- Three attempts, waiting 1 s and then 3 s; Cancel is honoured during the
  wait. Each retry is a line in the run log, naming the reason.
- A package that still failed is left out of the run only when the cache
  has no copy of it. A failed UPDATE of an installed package keeps the
  installed version: unticking it would make the plan uninstall it, which
  a failed download must never do.
- Scene 7 names what could not be downloaded, and the log opens, as it
  does whenever something went wrong.
- When NOTHING could be downloaded the run ends at the summary as before,
  listing what failed: there is nothing new to configure.
- Tested through the real path: a stand-in in front of the real server
  fails one download once (it must arrive on the retry) and another for
  good (it must be left out and named on scene 7).

### §7 — remembering the secret

DPAPI (`CryptProtectData` from crypt32, no crate), so another user or a
script walking the profile cannot read it. It is the one piece of state
that cannot live in the game folder, since it is not about one game.

## Things that are deliberately NOT done

Recorded so they are not rediscovered as oversights:

- **`SetUnhandledExceptionFilter`** for access violations and other faults
  that never reach a Rust panic hook. Revisit if a crash ever appears that
  leaves no panic line — that is exactly its signature.
- **Re-downloading a payload a restore needs but the cache no longer
  has.** The partial restore ships first; the preview already names what
  cannot come back.
- **Adopting a file the configuration step wrote that no package owns.**
  The step knows it wrote the file but not what was there before, and
  claiming it would let a later uninstall delete something the user always
  had. The consequence is accepted: a `PaceNote.ini` written from the
  template stays after everything is uninstalled. Fixing it properly means
  the step must report created-versus-edited.
- **An install root for the three games other than RBR.** Decided when
  that game gets packages; BeamNG's mods normally live under
  `%LOCALAPPDATA%`, not beside the exe.
