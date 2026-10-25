# server/CLAUDE.md — package + auth server

Single static binary. Runs anywhere (deployed on Linux). No size constraints —
normal async Rust. axum + tokio + libSQL. Packages are flat files on disk.

## Access model (no tiers)

Packages are grouped into **directories** — both an access boundary and a
literal on-disk level: `./data/<game>/packages/<directory>/<name>.zip`.

- `public/` is visible to everyone, credential or not.
- Every other directory is visible only to credentials **granted** it.
- One credential can hold grants for any number of directories.
- A grant covers the whole directory (list + download of everything in it).
  Per-package access = put the package in its own directory.
- Access directory ≠ UI category. The category is the FIRST segment of
  `name`, i.e. the folder below the access directory, and has NO effect on
  auth. A grant is per directory, whatever categories sit inside it.
- Access directory ≠ game either. The game is the level ABOVE `packages/`,
  so the same directory name may exist under several games and a grant for
  it covers that name in every game.

## Data model

- **Auth DB**: libSQL. Local embedded file `./data/auth.db` by default;
  switching to remote Turso must require ONLY changing `--db-url` /
  `PACENOTES_DB_URL` (keep all queries portable, no local-file assumptions).
  Tables: `tokens` + `grants(token_hash, directory)`. See
  migrations/0001_init.sql.
- **Data root** (`--data`, default `./data`): ONE server serves every game.

      data/<game>/schema.toml                       install schema (S11)
      data/<game>/packages/<directory>/<name>.zip   packages
      data/motd/<lang>.txt                          message of the day
      data/manifest.json                            every game, one document
      data/auth.db                                  credentials (libSQL)

  Every level means exactly one thing: game, then the literal `packages/`,
  then the access directory, then the package's `name` — its path below the
  access directory, nested as deep as the operator likes. Nothing positional
  comes from a sidecar, which is why `game` is NOT a sidecar key.
- **Install schema** (`<game>/schema.toml`, S11): `[[category]]` entries
  with `id` (a category folder), `select` (`one`|`any`) and `install_root`
  (a token like `game`, never a path); `deny_unknown_fields`, ids unique and
  `valid_id`. Publishing is STRICT: no schema → the game publishes nothing
  (error); unparsable → same; a package in an undeclared category → skipped
  (warning). The manifest embeds each schema with the SHA-256 of its RAW
  bytes — the value installers compare. The server bundles the repository's
  `schemas/<game>.toml` (`store::BUNDLED_SCHEMAS`) and warns when a data
  root's copy differs: installers of that release would refuse it.
  `motd` and `reports` are reserved data-root names, never games.
- **Packages**: game-root overlay zips, dumb payloads, never introspected by
  the server.
- **Sidecar metadata** (optional): `<name>.toml` next to its zip at whatever
  depth it lives, authored by hand. Position is NOT in here:

      display_name = "Numeric standard calls"   # renames the LEAF only
      exclusive_group = "pacenote-config"
      requires = ["public/language/polish"]     # "directory/name" refs
      description = "..."
      version = "2.0.0"                         # else "sha-<8 hex of sha256>"

  The sidecar is the ONLY source of these values (D5): nothing is inherited
  from a previous manifest, so the manifest is a pure function of zips +
  sidecars + schemas, and deleting a key removes it. `manifest gen` refuses
  (exit 1, writes nothing) when a run would DROP a value the current
  manifest carries, unless `--accept-metadata-loss`;
  `manifest migrate-to-sidecars` moves manifest-only values into sidecars.
  Unknown keys are an ERROR (`deny_unknown_fields`): an old sidecar carrying
  `category`, `tree_path` or `game` withholds ITS package instead of
  publishing silently-wrong metadata. The rest of the catalogue still
  publishes; the run exits 2.
- **manifest.json** (`shared::Manifest`) at `./data/manifest.json` is what
  `GET /packages` serves. It is GENERATED: `manifest gen` walks
  `<data>/<game>/packages/<directory>/**/*.zip`, computes
  size/sha256/updated_at, merges each sidecar, validates `requires` refs,
  and writes the file ATOMICALLY (temp file + rename in the data dir).
  Hand-edits get overwritten — edit sidecars instead. Exit status: 0 all
  published; 2 written but something withheld (`ERROR:` lines); 1 could not
  run (nothing written, the previous manifest stays).
  Publishing = drop zip (+ optional sidecar) → run `manifest gen`. No upload
  API in v1.
- Manifest is loaded at startup and hot-reloaded on file mtime change
  (checked lazily per request, cached with an RwLock — no inotify dep).
- Game ids and directory names: `[a-z0-9_-]+` (`shared::valid_id`), enforced
  by `manifest gen` and re-checked on manifest load. `public` is reserved for
  anonymous access and must never appear in `grants`.
- Package names: `shared::valid_name` — a `/`-separated path with no `\`, no
  `.`/`..` segment and no empty segment. Rejected or non-UTF-8 names are
  skipped with a warning rather than entering the manifest.
- `manifest gen` warns (does not fail) on the mis-filings that are easy to
  make: a category name where an access directory belongs, a category or
  access directory where a game belongs, a game folder without `packages/`,
  an unknown game id, a sidecar with no zip, and a name that needs
  percent-encoding in a URL.

## Security invariants

- Raw tokens are NEVER stored or logged. Store lowercase hex sha256 only.
  Mint format: `pcn_` + 32 random bytes base64url (admin CLI prints it once).
- Download path: `{game}`, `{directory}` and `{name}` from the URL are LOOKUP
  KEYS into the manifest, never joined into a filesystem path directly. Only a
  manifest hit yields `data/<game>/packages/<directory>/<name>.zip`. This
  structurally prevents path traversal and is what makes a nested, wildcard
  `{*name}` safe — keep it that way.
- `/motd`'s `lang` is the ONE path-ish input that cannot be validated by
  lookup, because it names a file no manifest lists. It is therefore
  restricted to `[a-z]{2,8}` before it touches the filesystem.
- Statuses never enumerate: on a non-public directory, no live key → 401
  WHETHER OR NOT the package exists; a live key without the grant, or no
  such package → 404 — the same answer. No 403. A syntactically impossible
  game/directory/name → 404 (nothing could exist). Body = `shared::ApiError`
  JSON; a 401 carries `WWW-Authenticate: Bearer`. `/packages` never 4xxs —
  it silently filters to public + granted directories.
- Token rows decode FAIL-CLOSED (an undecodable column is an error, never a
  default). Decode from `row.get_value`: libsql 0.6's typed `row.get::<i64>`
  PANICS on a type mismatch instead of returning an error.
- Public packages never touch the auth DB, and `/packages` falls back to the
  public listing on a DB error: a database blip must not take the free
  catalogue down. A bearer value not shaped like a minted key (`pcn_` + 43
  base64url chars) is rejected before any query.
- No panics in request handlers: anyhow at edges → `impl IntoResponse` error
  type mapping to status + ApiError.
- TLS is terminated by Caddy in front (deploy/Caddyfile). The binary listens
  plain HTTP on localhost/container network. Don't add TLS crates.

## Endpoints

- `GET /health` → 200 "ok"
- `GET /whoami` → 200 `shared::KeyStatus { expires_at }` for a live key,
  401 otherwise. How a client validates a key.
- `GET /packages[?game=<id>]` → shared::Manifest filtered to public +
  granted directories, and to one game when asked (absent = every game)
- `GET /download/{game}/{directory}/{*name}` → zip stream (tokio fs → axum
  body), Content-Length from the file handle (503 if it disagrees with the
  manifest), `application/zip`, Content-Disposition naming the LEAF with an
  ASCII `filename` and an RFC 6266 `filename*`; auth per access model above.
  `{*name}` is a wildcard: a name spans every remaining segment.
- `GET /motd?lang=xx` → `text/plain` from `data/motd/<lang>.txt`, `en`
  fallback, 404 when there is none, no auth
- `POST /report/conflict` (`reports.rs`) → `shared::ConflictReport`; 202
  empty / 400 / 413 / 429. Rate limit checked BEFORE parsing; body capped by
  `DefaultBodyLimit` on the route; nothing echoed; one JSON line per report
  in `data/reports/conflicts-YYYY-MM-DD.jsonl` with the server's
  `received_at`, never the client's address. `--trusted-proxy`
  (`PACENOTES_TRUSTED_PROXIES`) decides whose `X-Forwarded-For` counts.
  Requires serving with `into_make_service_with_connect_info::<SocketAddr>()`.
  `manifest gen` turns the last 30 days of reports into per-pair warnings.
- (v2, not now) POST /admin/* — credential management is a CLI subcommand

## CLI (clap)

- `pacenotes-server serve --listen 127.0.0.1:8080 --data ./data --db-url file:./data/auth.db`
- `pacenotes-server token mint --label "Jan" [--grant premium-voices]... [--expires-days N]`
  (repeatable --grant; a token with zero grants is valid but sees only public)
- `pacenotes-server token grant <token-or-hash> <directory>`
- `pacenotes-server token ungrant <token-or-hash> <directory>`
- `pacenotes-server token revoke <token-or-hash>`
- `pacenotes-server token list` (shows label, grants, expiry, revoked)
- `pacenotes-server manifest migrate-to-sidecars --data ./data` — one-off:
  copy values that exist only in manifest.json into sidecars (adds missing
  keys, never overwrites, never pins `0.0.0`/`sha-…`; idempotent).
- `pacenotes-server manifest gen --data ./data [--accept-metadata-loss] [--accept-unpublish]`
  — refuses (exit 1, writes nothing) when a game the current manifest serves
  would vanish because its schema is missing/broken, unless
  `--accept-unpublish`: an upgrade that forgot `schema.toml` must not empty
  the live catalogue. Scan + sidecar merge +
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
  PACENOTES_DATA_DIR).
- tracing for logs; never log Authorization headers. Every response gets
  one INFO line on target `access` (method, path+query, status, bytes, ms
  to the response head, 12-char token-HASH prefix or `-`/`malformed`, user
  agent) — production runs at INFO, where `TraceLayer` is silent.
- Cache headers: `/packages` `private, no-store` + `Vary: Authorization`;
  protected downloads `private, no-store`; public downloads `no-cache`.
  Every download carries `ETag: "<sha256>"` and honours `If-None-Match`
  with 304 — AFTER authorization, so a protected validator is never
  confirmed to a stranger.
- Keep handlers thin; logic in `auth.rs` / `store.rs` modules as they grow.
- Sidecar parsing needs a TOML crate (e.g. `toml`) — fine, the server has no
  size budget.
