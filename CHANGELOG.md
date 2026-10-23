# Changelog

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
