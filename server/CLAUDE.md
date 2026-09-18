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

## Data model

- **Auth DB**: libSQL. Local embedded file `./data/auth.db` by default;
  switching to remote Turso must require ONLY changing `--db-url` /
  `PACENOTES_DB_URL` (keep all queries portable, no local-file assumptions).
  Tables: `tokens` + `grants(token_hash, directory)`. See
  migrations/0001_init.sql.
- **Packages**: `./packages/<directory>/*.zip` + `./packages/manifest.json`
  (a `shared::Manifest`). The manifest is the source of truth for metadata
  (version, sha256, updated_at, description); each entry's `directory` field
  must equal its subdirectory. The DB is ONLY for auth. Publishing = drop a
  zip into a directory + run `manifest gen`. No upload API in v1.
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
- `pacenotes-server manifest gen --dir ./packages` — scans
  `<dir>/<directory>/*.zip`, computes sha256/size/mtime, sets each entry's
  `directory` from its subdir, preserves version/description of existing
  entries, rejects invalid directory names and top-level zips.

## Testing

- Unit: `auth::decide` (pure: token row + directory → decision) — exhaustive
  table test (exists in scaffold; extend, don't delete).
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
