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

- The Quadlet follows the host's existing pattern: every start clones the
  branch, builds the image and deletes the checkout, so
  `systemctl --user restart pacenotes` IS the deployment. Two deliberate
  departures from it:
  - `podman image prune -f` **without** `--external`. The `--external`
    form also deletes buildah's intermediate containers, which kills any
    concurrent podman build on the host.
  - `StartLimitIntervalSec=1h` with `StartLimitBurst=3`, because a restart
    loop re-runs a Rust release build each time.
- `CARGO_BUILD_JOBS=1` in the Containerfile: the host has 3 GB of RAM, and
  an OOM during the build presents as a vanishing-layer error rather than
  an honest message.
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
