# Server tasks (S1..S11)

## S1 — Compile + pin
- [x] `cargo build -p pacenotes-server`; fix any version breakage from the
      Cargo.toml majors; commit Cargo.lock
- [x] tracing-subscriber wired (env-filter, default info)

## S2 — Config + skeleton
- [x] clap CLI: `serve` subcommand with --listen/--packages/--db-url
      (+ env fallbacks per CLAUDE.md)
- [x] axum router: GET /health returns 200 "ok"; graceful shutdown on ctrl-c

## S3 — Manifest store
- [x] store.rs: load packages/manifest.json → shared::Manifest; RwLock cache;
      lazy mtime-based reload; validate directory names on load
- [x] GET /packages returns public-directory entries only (no auth yet)
- [x] `manifest gen` subcommand: scan <dir>/<subdir>/*.zip, subdir = access
      directory (validate [a-z0-9_-]+, reject top-level zips), sha256/size/
      updated_at
- [x] `manifest gen` sidecar merge: parse <name>.toml (category default
      "pacenotes", display_name, exclusive_group, requires, description,
      version); preserve version of existing entries when sidecar omits it
- [x] `manifest gen` validation + warnings: dangling requires, sidecar w/o
      zip, public package requiring non-public package
- [x] Unit tests: sidecar merge + validation cases

## S4 — Downloads
- [x] GET /download/{directory}/{name}: manifest (directory,name) lookup →
      stream zip (tokio::fs + axum Body), Content-Length + application/zip;
      404 unknown
- [x] Traversal test: directory+name are lookup keys, never paths (assert)

## S5 — libSQL auth store
- [x] migrations/0001_init.sql applied on startup (idempotent)
- [x] auth.rs: parse Bearer header → sha256 → load TokenRow + grants
      (single query, tokens LEFT JOIN grants)
- [x] `auth::decide` reviewed; `TokenRow::is_live` extracted so the
      /packages filter and decide() cannot drift apart

## S6 — Auth wiring
- [x] /packages: filter manifest to public + granted dirs (Manifest::visible_to);
      never 403 — filtering only (no directory-name enumeration)
- [x] /download: enforce decide(); 401/403 with shared::ApiError bodies
- [x] Never log tokens (tracing field audit; verified against a live run —
      neither the key nor the Authorization header reaches the log)

## S7 — Credential admin CLI
- [x] `token mint --label ... [--grant DIR]... [--expires-days N]`
      (pcn_ + 32B base64url, print once, store hash + grant rows)
- [x] `token grant` / `token ungrant` (reject directory "public")
- [x] `token revoke`, `token list` (label, grants, expiry, revoked)
- [x] expires_at + revoked honored by decide() (already in scaffold tests)

## S8 — Integration + deploy
- [x] Integration tests: {anon, no-grants, dirA, dirA+dirB} × {public, dirA,
      dirB} matrix for /packages filtering and /download statuses; expired
      token; revoked token; ungrant takes effect; 404; traversal
      (`server/tests/api.rs`, 9 tests on an ephemeral port + in-memory DB)
- [x] deploy/: Containerfile + entrypoint, Podman Quadlet
      (`pacenotes.container` + `pacenotes.config.env`, builds from git),
      Caddy site block, and `deploy/README.md` covering
      configuration, publishing, access keys, MOTD and troubleshooting
- [ ] Author sidecar .toml for the real packages (config variants share
      exclusive_group "pacenote-config"; voice packs require their language
      pack). NOT categories: those come from the folder layout now, and a
      `category` key in a sidecar is a hard error
- [ ] README snippet: publish flow = drop zip (+ sidecar) into
      packages/<dir>/ → `manifest gen` → done; access flow =
      `token mint --grant <dir>`

## S9 — Protocol rev. 2 server side (DESIGN.md rev. 2)
- [x] `manifest gen` recurses: `name` = the zip's path under its access
      directory, without `.zip`, `/`-separated at any depth. Validated with
      `pacenotes_shared::valid_name`; a name it rejects is skipped with a
      warning and never reaches the manifest.
- [x] Non-UTF-8 file names are skipped with a warning: a name that does not
      round-trip cannot be a lookup key.
- [x] `GET /download/{directory}/{*name}` is a wildcard route, so a nested
      name matches. The invariant is unchanged: directory+name are manifest
      keys, only a hit yields a path, every miss (traversal included) is 404.
- [x] `GET /motd?lang=xx` → `text/plain` from `<packages>/motd/<lang>.txt`,
      falling back to `en`, 404 when there is none, no auth. `lang` is
      restricted to `[a-z]{2,8}` BEFORE it touches the filesystem, because
      unlike `name` a query parameter cannot be validated by lookup.
- [x] `motd/` is skipped by `manifest gen`: it sits beside the access
      directories but is not one.
- [x] Mis-filing check: a top-level folder whose name is a known category
      (`codriver`, `language`, `style`, `pacenotes`) warns that the
      access-directory level is missing — which is exactly how the real
      `server/data` tree was filed.

## S9b — One server, every game

(Numbered S9b on 2026-10-25: it was a second "S10". Root `CLAUDE.md` and
`installer/TASKS.md` already cite S10 = conflict reports and S11 = install
schema, so the completed section moved rather than the open ones.)
- [x] Data root replaces the packages root: `--data` /`PACENOTES_DATA_DIR`
      (default `./data`), layout `<data>/<game>/packages/<directory>/<name>.zip`,
      `<data>/motd/<lang>.txt`, `<data>/manifest.json`.
- [x] `manifest gen` walks games; `PackageInfo::game` comes from the folder
      name, so `game` is no longer a sidecar key (an old sidecar carrying it
      fails the run via `deny_unknown_fields`).
- [x] `GET /packages?game=<id>` filters to one game; absent = every game in
      one document; an unknown game is an empty list, not an error.
- [x] `GET /download/{game}/{directory}/{*name}` — the game leads the
      address, because two games may hold the same directory+name.
      All three are lookup keys; a miss on any of them is 404.
- [x] `shared`: `valid_id` (games AND access directories, `[a-z0-9_-]+`),
      `KNOWN_GAMES`, `Manifest::for_game`, `PackageInfo::download_path`.
- [x] Mis-filing warnings: category where an access directory belongs,
      category/`public` where a game belongs, game folder without
      `packages/`, unknown game id, name needing percent-encoding.
- [x] Integration tests: two games holding the same `public/basic`, filtered
      listing, game as a lookup key, sidecar `game` rejected.
- [x] Real `server/data` re-filed to the layout (categories moved under
      `public/`); 24 packages publish with correct categories, a 28 MB
      nested download verified over HTTP.

## S3/S4 implementation notes

- Sidecar precedence is sidecar > previous manifest.json > protocol default,
  per field. That keeps `manifest gen` idempotent (a re-run never downgrades
  metadata) while letting a publisher change one field by touching one line.
  `size`/`sha256`/`updated_at` ignore both and always come from the file.
- A malformed sidecar is FATAL, not a warning: `deny_unknown_fields` plus a
  hard error means a typo (`categoy = "sounds"`) can never publish a package
  with silently-default metadata. Everything that is merely suspicious —
  sidecar without a zip, dangling/ill-formed `requires`, a public package
  requiring a protected one, a dependency belonging to another game — is a
  collected warning printed to stderr.
- `Generated { manifest, warnings }` exists so those warnings are testable
  rather than only observable in the log.
- A corrupt manifest.json never fails a request: the store logs and keeps
  serving the last good copy (empty on first load).
- Downloads stream via `tokio_util::io::ReaderStream`; Content-Length comes
  from the file handle, not the manifest, so a stale manifest cannot produce
  a truncated or over-long response.

## S5-S8 implementation notes

- `server` is now lib + bin: `src/lib.rs` exports the modules, `src/main.rs`
  is only the CLI. Integration tests could not otherwise drive the real
  router and the real admin commands — a test that reimplements them proves
  nothing.
- `TokenRow::is_live` is the single definition of "not revoked, not expired".
  `/packages` filtering and `auth::decide` both call it, so the listing and
  the download decision cannot disagree about a credential.
- `/packages` never 4xxs, by design: an expired, revoked, unknown or
  malformed credential silently gets the anonymous listing. Status codes
  there would let a caller enumerate directory names.
- `/download` resolves the package BEFORE deciding access, so a package that
  does not exist is 404 for everyone and the status never reveals which
  directories exist. Only a real package yields 401/403.
- Credentials are identified on the CLI by the raw key OR the hash (full or
  the 12-char prefix `token list` prints). An ambiguous prefix is an error,
  never a coin flip.
- `token mint` prints the key on stdout (pipeable) and the warning on
  stderr. It is the only place a raw key exists.
- Grants are read per request, never cached, so `ungrant`/`revoke` take
  effect immediately — asserted by tests, not just claimed.
- base64url is 20 hand-written lines with known-vector tests rather than a
  dependency.

## S9b implementation notes

- `game` could have been a sidecar key, a URL query or a folder level. The
  folder won for the same reason the tree did: one source of truth, and the
  operator's layout is already the thing they maintain. The cost is that
  moving a package between games is a `mv`, which is the point.
- The game HAD to enter the download URL. With one server, `public/basic`
  exists under every game, so `/download/{directory}/{*name}` would have
  been ambiguous — and resolving it by "first match" would silently serve
  another game's bytes. `PackageInfo::download_path()` builds the tail so
  both sides agree on the shape.
- `/packages` without `?game` returns every game deliberately: the installer
  is a multi-game wizard and one request is cheaper than four. The filter
  exists for clients that only care about one.
- Names may contain spaces (`style/Numeric Standard` does, on disk today).
  That is legal — it only has to survive a URL — so `manifest gen` warns and
  publishes rather than refusing. A client MUST percent-encode.

## S8 deploy notes

- The Quadlet follows the host's existing `pacenotes.container` pattern: every
  start clones the branch, builds the image and deletes the checkout, so
  `systemctl --user restart pacenotes` IS the deployment.
- Two deliberate departures from that pattern:
  - `podman image prune -f` WITHOUT `--external`. The `--external` form also
    deletes buildah's intermediate containers, which kills any concurrent
    podman build on the host. That is how the first attempts to build this
    image failed (`identifier is not a container`, `layer not known`,
    rustc reporting missing files under `target/`) while `pacenotes.service` was
    looping through its own build.
  - `StartLimitIntervalSec=1h` + `StartLimitBurst=3`, because a restart loop
    re-runs a Rust release build each time.
- `CARGO_BUILD_JOBS=1` in the Containerfile: the host has 3 GB of RAM, and an
  OOM during the build shows up as the same "vanishing layer" symptom rather
  than an honest error message.
- `libsql` lost its `replication` feature (tonic, tonic-web, prost-build,
  libsql_replication). Embedded replicas are not something this server
  creates; `core` + `remote` + `tls` still satisfy "moving auth to Turso is a
  --db-url change". `prost` remains via hrana, which `remote` requires.
- The container binds `0.0.0.0:8080`, not the CLI's `127.0.0.1` default — on
  loopback it would be unreachable from Caddy — and publishes no host port:
  Caddy terminates TLS and is the only route in.
- `entrypoint.sh` runs `manifest gen` before `serve`, so publishing is "copy
  the zip, restart the unit". A failed generation does not stop the server:
  it serves the previous manifest.

## S10 — Conflict reports from installers (decided 2026-10-25)

The installer no longer blocks a user when two selected packages write the
same file: the later one wins, it is logged, and the installer reports it
here. The reasoning is in `installer/CLAUDE.md` under "Overlapping files" —
only the operator can fix a catalogue that ships two packages fighting over
one path, and they cannot fix what they never hear about.

Payload revised in review (2026-10-25): flat `packages`/`files` lists lose
WHICH packages fought over WHICH file once a run has more than one conflict,
and the installer already knows every writer in install order. The client's
own timestamp is dropped: it is untrusted, so the server stamps the time.

- [ ] `POST /report/conflict`, no auth (an anonymous user hits conflicts
      too), `application/json`, body = `shared::ConflictReport`:

      { "game": "rbr", "installer": "0.2.0",
        "conflicts": [
          { "file": "Plugins/Pacenote/PaceNote.ini",
            "packages": ["public/language/polish",
                         "public/codriver/polish/male/Kamil"] } ] }

      `packages` are "directory/name" refs scoped by `game` (like
      `requires`), in install order — the last one won.
- [ ] Body capped at 8 KiB, ≤ 32 conflicts, 2..=8 packages each, `file`
      `valid_name` and ≤ 260 bytes, `installer` `[0-9A-Za-z.+-]{1,32}`. This
      is an unauthenticated write endpoint: treat every field as hostile,
      validate `game` with `valid_id` and the refs with `PackageRef::parse`,
      drop invalid entries (counted), and never echo any of it back.
      202 empty body; 400 when nothing valid remains; 413; 429.
- [ ] Rate limit per CLIENT: token bucket per IP plus a global ceiling. The
      server sits behind Caddy, so the peer address is Caddy's for every
      request: the client IP comes from `X-Forwarded-For`, honoured ONLY
      when the peer matches a configured `--trusted-proxy` (IP or CIDR);
      otherwise the peer address is the key. A header from anyone else is
      ignored, so it cannot be used to dodge the limit.
- [ ] Store append-only — one JSON line per report under
      `data/reports/conflicts-<date>.jsonl`, stamped with the server's
      `received_at`. No DB table: these are diagnostics, not state the
      server reasons about. `reports` is a reserved data-root name, so
      `manifest gen` never mistakes it for a game.
- [ ] `manifest gen` reads the last 30 days of reports and warns about
      package pairs reported as overlapping, which turns a user's bad day
      into a publishing-time check.
- [ ] Nothing in a report identifies a user: package refs and relative
      paths are catalogue data the server already has. No token, no
      hostname, no local paths — and no IP is stored (the rate limiter keeps
      addresses in memory only). Document it in `deploy/README.md` so the
      operator can answer the question honestly.

## S11 — Per-game install schema (decided 2026-10-25; replaces "manifest version")

Originally "the manifest carries a version". Redefined in review: what an
installer can fall behind on is not the package LIST — new packages in known
categories are just more of the same — but the shape of a game's catalogue:
which categories exist, which ask for a single pick, and where each
installs. That is a per-game schema, and only a change to it should stop an
installer.

- [ ] `data/<game>/schema.toml`, hand-editable:

      [[category]]
      id = "codriver"        # a top-level folder under the access directory
      select = "one"         # one = a choice the user must make; any = optional
      install_root = "game"  # a SYMBOLIC token the installer knows; never a path

      The canonical copy lives in the repository at `schemas/<game>.toml`
      (byte-exact: `.gitattributes` marks it `-text`), which is also what
      the installer embeds.
- [ ] `manifest gen` embeds each game's schema in manifest.json —
      `schemas: { "<game>": { sha256, categories } }` — where `sha256` is
      the hash of the file's RAW BYTES. The installer compares it with the
      hash of the schema it was built with. Any byte change, a comment
      included, therefore counts as a change: that is accepted, and is why
      the repository copy is the one to edit.
- [ ] The installer uses the schema as a COMPATIBILITY CHECK ONLY. Its
      behaviour — categories, single pick, install root, the localized
      labels — stays compiled in. A server can therefore not redirect where
      files are written, which keeps the root rule that a compromised server
      gains no installer behaviour.
- [ ] A mismatch blocks DOWNLOADING only, for that game only.
      Reconfiguring, repairing, uninstalling and restore keep working.
- [ ] Strict publishing: a game with no `schema.toml` publishes NOTHING; a
      schema that does not parse skips its game with an error; a package
      whose category the schema does not declare is skipped with a warning.
      Otherwise a category the installer cannot place slips through without
      the schema changing — the exact case this exists to catch.
- [ ] Rollout order for a schema change: release an installer that accepts
      BOTH the old and the new hash first, then switch the server's file.
      Switching the server first blocks every installer in the wild.

## S12 — Period-2 hardening from DESIGN-REVIEW.md (scheduled 2026-10-25)

Folded into the S10/S11 work at the maintainer's request. IDs point back to
`DESIGN-REVIEW.md`, where the reasoning is.

Auth correctness
- [x] **D3** No enumeration through status codes. Non-public path with no
      live token → 401 whether or not the package exists; live token without
      the grant, or no such package → 404. 403 is gone. (Keeps the
      installer's "anonymous protected download = 401" test true.)
- [x] **D8** Token rows decode fail-CLOSED. Found while testing it: libsql
      0.6's typed `row.get::<i64>` PANICS on a type mismatch rather than
      returning an error, so the old code would have killed the request on
      drift, not failed open. Decoding now goes through `get_value`.
- [x] **D9** Public downloads never touch the DB; `/packages` falls back to
      the public listing on a DB error; tokens not shaped `pcn_` + 43
      base64url characters are rejected before any query.
- [x] **D2** `GET /whoami`: 200 `{expires_at}` for a live key, 401
      otherwise — the only way an installer can tell a typo from a valid key.

Robustness
- [x] **D10** SIGTERM handled, drain bounded to 20 s. Verified live: an
      idle stop takes 0.01 s; a stop with a 250-second download in flight
      ends at 20.0 s, exit 0. The Quadlet sets `StopTimeout=30`, because
      podman's default 10 s would SIGKILL mid-drain.
- [x] **S5** `manifest.json` written atomically; a broken sidecar or schema
      skips only its package or game and the run exits non-zero; the
      entrypoint says so loudly.
- [x] **S7** Local DB: WAL + `busy_timeout`.
- [x] **S4** (minimal) A file whose size differs from the manifest is 503,
      not a short body that fails the client's sha check.

Observability and HTTP hygiene
- [ ] **D11** One INFO access-log line per response (token-hash prefix,
      never the token).
- [ ] **D12** `Content-Disposition` names the leaf, with RFC 6266
      `filename*`.
- [ ] **C2** Cache headers: `/packages` private/no-store + `Vary:
      Authorization`; protected downloads private/no-store; public
      downloads `ETag` + `If-None-Match` → 304.

Publishing purity
- [ ] **D5** The manifest is a pure function of zips + sidecars + schemas.
      A sidecar without `version` gets `sha-<first 8 hex of sha256>`.
      `manifest gen` refuses to drop metadata the current manifest carries
      unless told to; `manifest migrate-to-sidecars` moves it into sidecars.
