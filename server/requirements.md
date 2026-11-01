# Server requirements

What server work has to satisfy. The RULES that constrain the code are in
`server/CLAUDE.md`; the protocol itself is in the root `CLAUDE.md`; how
work is verified is in the root `requirements.md`.

## Publishing (`manifest gen`)

- Field precedence is **sidecar > the previous `manifest.json` > the
  protocol default**, per field. That keeps a re-run idempotent — it can
  never downgrade metadata — while letting a publisher change one field by
  touching one line. `size`, `sha256` and `updated_at` ignore both and
  always come from the file.
- A malformed sidecar is **fatal, not a warning**. `deny_unknown_fields`
  plus a hard error means a typo such as `categoy = "sounds"` can never
  publish a package with silently defaulted metadata.
- Everything merely suspicious is a collected warning on stderr: a sidecar
  with no zip, a dangling or ill-formed `requires`, a public package
  requiring a protected one, a dependency in another game.
- Warnings are returned as data (`Generated { manifest, warnings }`) so
  they can be tested rather than only observed in a log.
- The manifest is a pure function of zips, sidecars and schemas. A sidecar
  without `version` gets `sha-<first 8 hex of sha256>`.

## Serving

- A corrupt `manifest.json` never fails a request: log it and keep serving
  the last good copy.
- Downloads stream, and `Content-Length` comes from the file handle rather
  than the manifest — a stale manifest must not be able to produce a
  truncated or over-long response.
- `/packages` never returns 4xx. An expired, revoked, unknown or malformed
  credential silently gets the anonymous listing, because status codes
  there would let a caller enumerate directory names.
- `/download` resolves the package BEFORE deciding access, so a package
  that does not exist is 404 for everyone.
- `TokenRow::is_live` is the single definition of "not revoked, not
  expired". The listing filter and the download decision both call it, so
  they cannot drift apart.

## Shape of the crate

- `server` is a lib plus a bin: `src/lib.rs` exports the modules,
  `src/main.rs` is only the CLI. Integration tests drive the real router
  and the real admin commands — a test that reimplements them proves
  nothing.
- Credentials are identified on the CLI by the raw key or by the hash
  (full, or the 12-character prefix `token list` prints). An ambiguous
  prefix is an error, never a coin flip.
- `token mint` prints the key on stdout and the warning on stderr. It is
  the only place a raw key exists.
- Grants are read per request and never cached, so `ungrant` and `revoke`
  take effect immediately.
- base64url is twenty hand-written lines with known-vector tests, not a
  dependency.

## Deploy

- Images are built in CI, never on the host. A `v/X.Y.Z` tag runs
  `.github/workflows/cd-release-on-tag.yml`, which pushes
  `ghcr.io/<owner>/pacenotes-server:X.Y.Z` (and `:sha-<12>`) as a PRIVATE
  package — never a release asset or workflow artifact, because the
  repository is public and so are its artifacts — then SSHes to the host
  to run `deploy/pacenotes-update X.Y.Z`.
- The Quadlet runs the LOCAL tag `localhost/pacenotes-server:current` with
  `Pull=never`; only `pacenotes-update` moves that tag. A restart therefore
  never builds, never pulls and never changes the version, and works with
  the registry down.
- `pacenotes-update` is the deploy key's forced `command=`. It takes ONLY an
  `X.Y.Z` version from `SSH_ORIGINAL_COMMAND`, pulls that exact image with
  the host's own `read:packages` token, keeps the running image as
  `:previous`, restarts, and waits for the healthcheck; an unhealthy
  container is rolled back to `:previous` and the script exits 1, failing
  the CI job.
- Not `podman auto-update` and no `AutoUpdate=`: it cannot be scoped to one
  unit, and other containers on the host carry `AutoUpdate`, so every
  release would have updated them too.
- `podman untag` needs the image AND the name: given a name alone it strips
  every name the image has, `:current` included, and the prune that follows
  deletes the running image. Found by the script's test, 2026-11-01.
- `podman image prune -f` **without** `--external`: the `--external` form
  deletes buildah's intermediate containers, killing any concurrent build.
- `StartLimitIntervalSec=1h` with `StartLimitBurst=3`: a crash loop stops
  instead of restarting forever.
- `CARGO_BUILD_JOBS=1` is the Containerfile's DEFAULT, for a build on the
  3 GB host, where an OOM presents as a vanishing-layer error rather than an
  honest message. CI passes `CARGO_BUILD_JOBS=4`.
- `libsql` is built without its `replication` feature. Embedded replicas
  are not something this server creates, and `core` + `remote` + `tls`
  still satisfy "moving auth to Turso is a `--db-url` change".
- The container binds `0.0.0.0:8080` and publishes no host port: Caddy
  terminates TLS and is the only route in.
- `entrypoint.sh` runs `manifest gen` before `serve`. A failed generation
  does not stop the server; it serves the previous manifest.

## Design decisions worth not relitigating

- **`game` comes from the folder level**, not a sidecar key or a query
  parameter — one source of truth, and the operator's layout is the thing
  they already maintain. Moving a package between games is a `mv`, which
  is the point.
- **The game leads the download URL.** With one server, `public/basic` can
  exist under every game, so a game-less path would be ambiguous and
  resolving it by first match would silently serve another game's bytes.
- **`/packages` without `?game` returns every game**, because the
  installer is a multi-game wizard and one request is cheaper than four.
  The filter exists for clients that care about one.
- **Names may contain spaces** (`style/Numeric Standard` does). That is
  legal — it only has to survive a URL — so `manifest gen` warns and
  publishes rather than refusing. Clients must percent-encode.
- **A schema change needs an installer release first.** Ship an installer
  that accepts both the old and the new hash, then switch the server's
  file. Switching the server first blocks every installer in the wild.
