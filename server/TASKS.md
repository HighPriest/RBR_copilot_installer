# Server tasks (S1..S8)

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
      exclusive_group "pacenote-config"; voice packs requires their
      language pack; categories: sounds/configs/languages/pacenotes)
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

## S10 — One server, every game
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

## S10 implementation notes

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
