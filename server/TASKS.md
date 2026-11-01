# Server tasks (S1..S12)

Specs are in `server/requirements.md`, rules in `server/CLAUDE.md`, the
protocol in the root `CLAUDE.md`.

## S1 — Compile and pin

- [x] Build `pacenotes-server` and commit the lockfile
- [x] Wire tracing-subscriber with an env filter, defaulting to info

## S2 — Config and skeleton

- [x] Add a clap CLI: `serve` with `--listen`, `--packages`, `--db-url`
      and env fallbacks
- [x] Add the axum router, `GET /health`, and graceful shutdown on ctrl-c

## S3 — Manifest store

- [x] Load `manifest.json` into `shared::Manifest` behind an RwLock, with
      a lazy mtime-based reload and directory-name validation
- [x] Serve `GET /packages` with public entries only
- [x] Add `manifest gen`: scan the access directories for zips, record
      sha256, size and `updated_at`
- [x] Merge sidecars in `manifest gen`
- [x] Warn on dangling `requires`, a sidecar without a zip, and a public
      package requiring a protected one
- [x] Unit-test the merge and the validation cases

## S4 — Downloads

- [x] Serve `GET /download/{directory}/{name}` as a stream, with
      Content-Length and the zip content type
- [x] Assert that directory and name are lookup keys and never paths

## S5 — libSQL auth store

- [x] Apply `migrations/0001_init.sql` idempotently at startup
- [x] Parse the bearer header, hash it, and load the token row with its
      grants in one query
- [x] Extract `TokenRow::is_live` so the listing filter and `decide`
      cannot drift apart

## S6 — Auth wiring

- [x] Filter `/packages` to public plus granted directories, never 403
- [x] Enforce `decide` on `/download`
- [x] Never log tokens, and verify it against a live run

## S7 — Credential admin CLI

- [x] `token mint` with label, grants and expiry, printing the key once
- [x] `token grant` and `token ungrant`, rejecting `public`
- [x] `token revoke` and `token list`
- [x] Honour `expires_at` and `revoked` in `decide`

## S8 — Integration and deploy

- [x] Integration-test the {anon, no grants, dirA, dirA+dirB} ×
      {public, dirA, dirB} matrix, plus expired, revoked, ungrant, 404 and
      traversal
- [x] Write the deploy set: Containerfile, entrypoint, Podman Quadlet,
      Caddy site block and `deploy/README.md`
- [ ] Author the sidecar `.toml` files for the real packages
  - [ ] Share `exclusive_group = "pacenote-config"` across the config
        variants, and make each voice pack require its language pack
  - [ ] Set no categories: they come from the folder layout, and a
        `category` key in a sidecar is a hard error
- [ ] Add a README snippet for the publish and access flows

## S9 — Protocol rev. 2

- [x] Make `manifest gen` recurse, so `name` is the zip's path under its
      access directory at any depth
- [x] Skip names that `valid_name` rejects, with a warning
- [x] Skip non-UTF-8 file names with a warning
- [x] Make `/download/{directory}/{*name}` a wildcard route
- [x] Serve `GET /motd?lang=xx` with an `en` fallback and no auth
  - [x] Restrict `lang` to `[a-z]{2,8}` before it reaches the filesystem
- [x] Skip `motd/` in `manifest gen`
- [x] Warn when a top-level folder is a known category, which means the
      access-directory level is missing

## S9b — One server, every game

- [x] Replace the packages root with a data root, laid out
      `<data>/<game>/packages/<directory>/<name>.zip`
- [x] Walk games in `manifest gen` and take `game` from the folder name
  - [x] Reject `game` as a sidecar key
- [x] Filter `GET /packages?game=<id>`, treating an unknown game as empty
- [x] Lead the download route with the game
- [x] Add `valid_id`, `KNOWN_GAMES`, `Manifest::for_game` and
      `PackageInfo::download_path` to `shared`
- [x] Warn on every mis-filing case
- [x] Integration-test two games holding the same directory and name
- [x] Re-file the real `server/data` tree to the layout

## S10 — Conflict reports from installers

- [x] Accept `POST /report/conflict` without auth, body
      `shared::ConflictReport`, packages in install order
- [x] Treat every field as hostile: cap the body at 8 KiB, bound the
      counts, validate the ids and refs, drop invalid entries, echo
      nothing back
- [x] Rate-limit per client and globally
  - [x] Take the client from `X-Forwarded-For` only when the peer is a
        configured `--trusted-proxy`
- [x] Store append-only as JSONL under `data/reports/`, stamped with the
      server's own time
  - [x] Reserve `reports` as a data-root name
- [x] Warn in `manifest gen` about pairs reported in the last 30 days
- [x] Keep every report free of user identity, and document that in
      `deploy/README.md`

## S11 — Per-game install schema

- [x] Read `data/<game>/schema.toml`: the categories, which ask for one
      pick, and a symbolic `install_root`
  - [x] Keep the canonical copy at `schemas/<game>.toml`, byte-exact
- [x] Embed each game's schema and the sha256 of its raw bytes in the
      manifest
- [x] Publish strictly: no schema means the game publishes nothing, an
      unparsable schema skips the game, an undeclared category skips the
      package
- [x] Document the rollout order in `deploy/README.md`

## S12 — Period-2 hardening

IDs point into `DESIGN-REVIEW.md`.

- [x] Stop enumeration through status codes: 401 without a live key, 404
      with one, no 403 at all (D3)
- [x] Make token rows decode fail-closed (D8)
  - [x] Go through `get_value`: libsql's typed getter panics on a type
        mismatch rather than returning an error
- [x] Keep public downloads off the DB, fall back to the public listing on
      a DB error, and reject malformed tokens before any query (D9)
- [x] Add `GET /whoami` (D2)
- [x] Handle SIGTERM with a 20-second bounded drain (D10)
  - [x] Set `StopTimeout=30` in the Quadlet, since podman's 10 s would
        SIGKILL mid-drain
- [x] Write `manifest.json` atomically, skip only the broken package or
      game, and exit non-zero (S5)
- [x] Turn on WAL and `busy_timeout` for the local DB (S7)
- [x] Return 503 when a file's size differs from the manifest (S4)
- [x] Log one INFO line per response, with a token-hash prefix (D11)
- [x] Name the leaf in `Content-Disposition`, RFC 6266 (D12)
- [x] Set cache headers: no-store and `Vary: Authorization` on
      `/packages`, `ETag` and 304 on public downloads (C2)
- [x] Make the manifest a pure function of zips, sidecars and schemas (D5)
  - [x] Derive a missing version as `sha-<8 hex>`
  - [x] Refuse to drop metadata unless told to, and add
        `manifest migrate-to-sidecars`
- [x] Write the log without colour codes when stdout is not a terminal
