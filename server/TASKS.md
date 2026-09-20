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
- [ ] migrations/0001_init.sql applied on startup (idempotent)
- [ ] auth.rs: parse Bearer header → sha256 → load TokenRow + grants
      (single query, tokens LEFT JOIN grants)
- [ ] `auth::decide` already implemented + table-tested in scaffold: review,
      extend cases if behavior grows

## S6 — Auth wiring
- [ ] /packages: filter manifest to public + granted dirs (Manifest::visible_to);
      never 403 — filtering only (no directory-name enumeration)
- [ ] /download: enforce decide(); 401/403 with shared::ApiError bodies
- [ ] Never log tokens (tracing field audit)

## S7 — Credential admin CLI
- [ ] `token mint --label ... [--grant DIR]... [--expires-days N]`
      (pcn_ + 32B base64url, print once, store hash + grant rows)
- [ ] `token grant` / `token ungrant` (reject directory "public")
- [ ] `token revoke`, `token list` (label, grants, expiry, revoked)
- [ ] expires_at + revoked honored by decide() (already in scaffold tests)

## S8 — Integration + deploy
- [ ] Integration tests: {anon, no-grants, dirA, dirA+dirB} × {public, dirA,
      dirB} matrix for /packages filtering and /download statuses; expired
      token; revoked token; ungrant takes effect; 404; traversal
- [ ] deploy/: Caddyfile (reverse_proxy + auto-HTTPS), systemd unit or
      Dockerfile (static musl build optional)
- [ ] Author sidecar .toml for the real packages (config variants share
      exclusive_group "pacenote-config"; voice packs requires their
      language pack; categories: sounds/configs/languages/pacenotes)
- [ ] README snippet: publish flow = drop zip (+ sidecar) into
      packages/<dir>/ → `manifest gen` → done; access flow =
      `token mint --grant <dir>`

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
