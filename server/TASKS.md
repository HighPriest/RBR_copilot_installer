# Server tasks (S1..S8)

## S1 — Compile + pin
- [ ] `cargo build -p pacenotes-server`; fix any version breakage from the
      Cargo.toml majors; commit Cargo.lock
- [ ] tracing-subscriber wired (env-filter, default info)

## S2 — Config + skeleton
- [ ] clap CLI: `serve` subcommand with --listen/--packages/--db-url
      (+ env fallbacks per CLAUDE.md)
- [ ] axum router: GET /health returns 200 "ok"; graceful shutdown on ctrl-c

## S3 — Manifest store
- [ ] store.rs: load packages/manifest.json → shared::Manifest; RwLock cache;
      lazy mtime-based reload; validate directory names on load
- [ ] GET /packages returns public-directory entries only (no auth yet)
- [ ] `manifest gen` subcommand: scan <dir>/<subdir>/*.zip, subdir = access
      directory (validate [a-z0-9_-]+, reject top-level zips), sha256/size/
      mtime; preserve version/description of existing entries

## S4 — Downloads
- [ ] GET /download/{directory}/{name}: manifest (directory,name) lookup →
      stream zip (tokio::fs + axum Body), Content-Length + application/zip;
      404 unknown
- [ ] Traversal test: directory+name are lookup keys, never paths (assert)

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
- [ ] README snippet: publish flow = drop zip into packages/<dir>/ →
      `manifest gen` → done; access flow = `token mint --grant <dir>`
