# Changelog

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
