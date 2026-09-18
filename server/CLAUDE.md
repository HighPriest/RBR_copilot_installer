# server/CLAUDE.md — package + auth server

Single static binary. Runs anywhere (deployed on Linux). No size constraints —
normal async Rust. axum + tokio + libSQL. Packages are flat files on disk.

## Access model (no tiers)

Packages are grouped into **directories** — both an access boundary and the
literal on-disk subdirectory: `./packages/<directory>/<name>.zip`.

- `public/` is visible to everyone, credential or not.
- Every other directory is visible only to credentials **granted** it.
- One credential can hold grants for any number of directories.
- A grant covers the whole directory (list + download of everything in it).
  Per-package access = put the package in its own directory.
- Access directory ≠ UI category. `PackageInfo.category` (sounds/configs/
  languages/pacenotes/...) is presentation metadata and lives in the
  manifest; it has NO effect on auth.

## Data model

- **Auth DB**: libSQL. Local embedded file `./data/auth.db` by default;
  switching to remote Turso must require ONLY changing `--db-url` /
  `PACENOTES_DB_URL` (keep all queries portable, no local-file assumptions).
  Tables: `tokens` + `grants(token_hash, directory)`. See
  migrations/0001_init.sql.
- **Packages**: `./packages/<directory>/<name>.zip` — game-root overlay
  zips, dumb payloads, never introspected by the server.
- **Sidecar metadata** (optional): `./packages/<directory>/<name>.toml`
  next to its zip, authored by hand:

      category = "configs"            # default if absent: "pacenotes"
      display_name = "Numeric standard calls"
      exclusive_group = "pacenote-config"
      requires = ["public/english"]   # "directory/name" refs
      description = "..."
      version = "2.0.0"               # optional; else keep existing/1.0.0

- **manifest.json** (`shared::Manifest`) at `./packages/manifest.json` is
  what `GET /packages` serves. It is GENERATED: `manifest gen` scans
  `<dir>/<subdir>/*.zip`, computes size/sha256/updated_at, merges each zip's
  sidecar .toml, validates `requires` refs point at existing packages, and
  writes the file. Hand-edits to manifest.json get overwritten — edit
  sidecars instead. Publishing = drop zip (+ optional sidecar) → run
  `manifest gen`. No upload API in v1.
- Manifest is loaded at startup and hot-reloaded on file mtime change
  (checked lazily per request, cached with an RwLock — no inotify dep).
- Directory names: `[a-z0-9_-]+`, enforced by `manifest gen` and re-checked
  on manifest load. `public` is reserved for anonymous access and must never
  appear in `grants`.

## Security invariants

- Raw tokens are NEVER stored or logged. Store lowercase hex sha256 only.
  Mint format: `pcn_` + 32 random bytes base64url (admin CLI prints it once).
- Download path: `{directory}` and `{name}` from the URL are LOOKUP KEYS into
  the manifest, never joined into a filesystem path directly. Only a manifest
  hit yields `packages/<directory>/<name>.zip`. This structurally prevents
  path traversal — keep it that way.
- 401 (missing/bad/expired/revoked token on a protected directory) vs 403
  (valid token, directory not granted) vs 404 (no such package). Body =
  `shared::ApiError` JSON. NOTE: /packages never 403s — it silently filters
  to public + granted directories, so callers can't enumerate directory names.
- No panics in request handlers: anyhow at edges → `impl IntoResponse` error
  type mapping to status + ApiError.
- TLS is terminated by Caddy in front (deploy/Caddyfile). The binary listens
  plain HTTP on localhost/container network. Don't add TLS crates.

## Endpoints

- `GET /health` → 200 "ok"
- `GET /packages` → shared::Manifest filtered to public + granted directories
- `GET /download/{directory}/{name}` → zip stream (tokio fs → axum body),
  correct Content-Length, `application/zip`; auth per access model above
- (v2, not now) POST /admin/* — credential management is a CLI subcommand

## CLI (clap)

- `pacenotes-server serve --listen 127.0.0.1:8080 --packages ./packages --db-url file:./data/auth.db`
- `pacenotes-server token mint --label "Jan" [--grant premium-voices]... [--expires-days N]`
  (repeatable --grant; a token with zero grants is valid but sees only public)
- `pacenotes-server token grant <token-or-hash> <directory>`
- `pacenotes-server token ungrant <token-or-hash> <directory>`
- `pacenotes-server token revoke <token-or-hash>`
- `pacenotes-server token list` (shows label, grants, expiry, revoked)
- `pacenotes-server manifest gen --dir ./packages` — scan + sidecar merge +
  validation as described under Data model. Warnings for: sidecar without
  zip, invalid directory name, top-level zips, dangling `requires`,
  cross-directory `requires` pointing into a NON-public directory from a
  public package (a public package must not require protected content).

## Testing

- Unit: `auth::decide` (pure: token row + directory → decision) — exhaustive
  table test (exists in scaffold; extend, don't delete).
- Unit: sidecar merge + `requires` validation in `manifest gen`.
- Integration: spin server on an ephemeral port with a tempdir packages tree
  (public/ + two protected dirs) + in-memory libsql; assert the
  {anon, token-no-grants, token-dirA, token-dirA+dirB} × {public, dirA, dirB}
  matrix for both /packages filtering and /download status codes, plus 404,
  expired/revoked, and traversal attempts
  (`/download/..%2F..%2Fdata/auth.db` must 404).

## Conventions

- All config via flags with env fallbacks (PACENOTES_LISTEN, PACENOTES_DB_URL,
  PACENOTES_PACKAGES_DIR).
- tracing for logs; never log Authorization headers.
- Keep handlers thin; logic in `auth.rs` / `store.rs` modules as they grow.
- Sidecar parsing needs a TOML crate (e.g. `toml`) — fine, the server has no
  size budget.
