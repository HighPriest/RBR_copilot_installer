# Project task list (ordered milestones)

Work top to bottom. Each milestone leaves the repo buildable. Per-crate
detail is in `installer/TASKS.md` and `server/TASKS.md`; what a task must
satisfy before it counts as done is in `requirements.md`.

## M0 — Repo bootstrap

- [x] Lay out the workspace: `shared/`, `installer/`, `server/`
- [x] Write the CLAUDE.md files (root, installer, server)
- [x] Copy the legacy C# sources into `legacy/` for reference
- [x] `git init` and a first commit
- [x] Pin every dependency to an exact version and commit `Cargo.lock`
- [x] Port the legacy GitHub Actions workflows (`ci.yml`, plus the three
      `cd-*` deploy workflows)
  - [x] Give the installer its own CI job: it is excluded from the
        workspace, so the server job's fmt and clippy never reached it
  - [x] Run fmt, clippy, tests, version sync and the size gate there
  - [x] Leave the GUI tests out of CI — they need an interactive desktop

## M1 — Shared protocol crate

- [x] Make `PackageInfo`, `Manifest` and `ApiError` compile
- [x] Unit-test the JSON round trip and `Manifest::visible_to` filtering
- [x] Get `cargo test -p pacenotes-shared` green

## M1a — Protocol v2: install metadata

See root `CLAUDE.md`, "Install model".

- [x] Extend `PackageInfo` with `category`, `display_name`,
      `exclusive_group` and `requires`
  - [x] Add `game` as well, so one server can serve four games without
        every package appearing under every one
- [x] Unit-test that old manifests still deserialize, and that absent
      optionals are not serialized
- [x] Add `shared::PackageRef`, `PackageInfo::label()`, `required_refs()`
      and `Manifest::get()`

## M2 — Server MVP (real files, no auth yet)

- [x] Work through `server/TASKS.md` S1..S4
- [x] Serve `GET /health`, `GET /packages` and `GET /download/...`
- [x] Check by hand with curl: health, anonymous listing, a byte-identical
      download, a 404 for an unknown package, and a rejected traversal

## M3 — Server auth

- [x] Work through `server/TASKS.md` S5..S7
- [x] Add the libSQL schema and migrations, token hashing and the
      directory-grant check
- [x] Add the admin CLI: mint and revoke credentials
- [x] Add the admin CLI: grant and ungrant directories
- [x] Integration-test the listing and download matrix across
      {anon, no grants, one directory, several}
  - [x] Cover expired, revoked, ungranted, 404 and traversal while the
        harness is there

## M4 — Installer skeleton (Windows)

- [x] Work through `installer/TASKS.md` I1 (toolchain, resources, size gate)
- [x] Work through I2 and I2a (the UI per `installer/DESIGN.md`)
- [x] Work through I3 (the WinHTTP client and `--selftest-http`)
- [x] Open a Win32 dialog and pass the release size gate
- [x] Fetch `GET /packages` through WinHTTP against a real server
  - [x] Cover bearer auth and the 401, 404 and unreachable surfaces
  - [x] Keep it desktop-free so CI can run it

## M5 — Installer cache and install engine

- [x] Work through I4 (worker thread, progress and cancel)
- [x] Work through I5 (api, std-only sha256, sha-keyed cache, verified
      downloads)
- [x] Build the download scene from where packages sit on the server, and
      show each leaf's state
- [x] Build the configuration, progress and summary scenes
- [x] Work through I6 (zip reader)
- [x] Work through I7: the ledger, overlay extraction, one-time backup of
      files the ledger does not own, and uninstall from the ledger
- [x] Make Apply a diff of selection against ledger, honouring exclusive
      groups and enforcing `requires`
- [x] Give each game its own configuration step, with no shared
      abstraction over them
  - [x] RBR: edit the existing Pacenote config, preserving the rest
- [x] Split downloading and installing into two screens with the
      configuration step between them, and offer both routes on scene 3
  - [x] Not in the original plan: a download fills a cache, an install
        writes to the game folder, and a user should be able to do either
        without the other

## M6 — Installer auth and polish

- [x] Work through I8: use the access key as the bearer for every request
  - [x] Validate it with `GET /whoami`, the only endpoint that can
  - [x] Warn when a key is close to expiring, naming the date
- [x] Build the restore scene: go back to any earlier state, or remove
      everything, after seeing exactly what that would do
- [ ] Work through `installer/TASKS.md` I9 (polish) and I10 (release gate)
- [ ] Audit the final size against the 620 kB gate
  - [x] Save a `cargo bloat` report to `docs/bloat.txt`

## M7 — Ship

- [x] Write the deploy set: Containerfile, Podman Quadlet, Caddy site
      block and `deploy/README.md`
- [ ] Build and push the image from CI, tagged with the git sha, so the
      Quadlet pins a tag and a restart never rebuilds
  - [ ] Build both the server and the installer in CI
  - [ ] Store the build artifacts
  - [ ] Source the executable from CI when the Quadlet deploys
  - [ ] Make publishing content mean running `manifest gen` alone, and say
        so in `deploy/README.md`
- [ ] Author the sidecar `.toml` files for the real packages: display
      names, `exclusive_group` on the config variants, `requires` on the
      voice packs
- [ ] Write reproducible release-build instructions for both binaries
- [ ] Smoke-test on a fresh Windows VM: download, install a subset, switch
      a config variant, uninstall, restore

### Supply chain and operations

From the design review's period 2. The findings themselves stay in
`DESIGN-REVIEW.md`.

- [ ] Sign the `.exe` with Authenticode in CD and publish its SHA-256 (O2)
- [ ] Pin the toolchain and the base images by digest (O2)
- [ ] Publish a versioned server artifact rather than a branch tip (O2)
- [ ] Bind Caddy's plain-HTTP block to loopback, or remove it (O2)
- [ ] Make the `EnvironmentFile` path agree with `deploy/README.md` (O2)
- [ ] Confirm the default hostname stays under the project's control, and
      never ship a provider hostname (C3)
- [ ] Back up `auth.db` and the packages themselves (S9)
- [ ] Document publishing by copy-to-`.tmp`-then-rename (S4)
- [ ] Document that renaming or moving a package folder is a breaking
      change, because the folder path is the package's identity (P13)
