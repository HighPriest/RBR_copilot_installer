# Project task list (ordered milestones)

Work top-to-bottom. Each milestone leaves the repo in a buildable state.
Detailed per-crate steps live in `installer/TASKS.md` and `server/TASKS.md`.

## M0 — Repo bootstrap
- [x] Workspace layout (`shared/`, `installer/`, `server/`)
- [x] CLAUDE.md files (root, installer, server)
- [x] Copy legacy C# sources into `legacy/` for reference
- [x] `git init`, first commit
- [x] Pin all `*`/loose versions in Cargo.toml files to exact versions
      (`cargo add` latest stable of each; commit Cargo.lock)
- [x] Setup github actions scripts, equivalent to `legacy\.github\workflows`
      (`ci.yml` + ported `cd-debugbuild-manually` / `cd-deploy-manually` /
      `cd-deploy-on-pull-request`)
- [x] CI covers the installer too: because it is excluded from the workspace,
      the `server` job's fmt/clippy never reached it, so the installer job now
      runs fmt + clippy + tests + version sync + the size gate. The GUI tests
      need an interactive desktop and stay local (`tests/Run-AllTests.ps1`).

## M1 — Shared protocol crate
- [x] `shared`: PackageInfo (directory-based), Manifest, ApiError compile
- [x] Unit tests: JSON round-trip; Manifest::visible_to grant filtering
- [x] `cargo test -p pacenotes-shared` green

## M1a — Protocol v2: install metadata (see root CLAUDE.md "Install model")
- [x] Extend PackageInfo: `category: String`,
      `display_name: Option<String>`, `exclusive_group: Option<String>`,
      `requires: Vec<String>` ("directory/name" refs; serde default = empty)
- [x] ALSO `game: String` ("rbr"/"dirtrally"/"beamng", serde default "rbr").
      Not in the original list: `installer/DESIGN.md` made the app multi-game
      after this was written, and without the field every package would show
      under every ribbon tab. Constants `GAME_*` mirror
      `installer/src/gamedir.rs`.
- [x] Unit tests: round-trip incl. new fields; old manifests without the
      fields still deserialize (serde defaults; `category` defaults to
      "pacenotes" via #[serde(default = ...)]); absent optionals are not
      serialized, so a v1 reader still sees a v1 document
- [x] `shared::PackageRef` helper: parse/format "directory/name" (rejects
      `\\`, empty halves and extra separators, so a ref never widens into a
      path); `PackageInfo::label()`, `required_refs()`, `Manifest::get()`

## M2 — Server MVP (serves real files, no auth yet)
- [x] server/TASKS.md → S1..S4
- [x] `GET /health`, `GET /packages` (public only),
      `GET /download/{directory}/{name}`
- [x] Manual test with curl against a `packages/` dir with one test zip
      (health 200, anon listing shows only `public`, download byte-identical
      with correct Content-Length/Type/Disposition, unknown package 404,
      `..%2F..%2Fdata/auth.db` 404)

(The M2 gap where /download was unauthenticated is closed by M3 below.)

## M3 — Server auth
- [x] server/TASKS.md → S5..S7
- [x] libSQL schema + migrations (tokens + grants), token hashing,
      directory-grant checks (auth::decide)
- [x] Admin CLI subcommand: mint/revoke credentials
- [x] Admin CLI: grant/ungrant directories per credential
- [x] Integration tests: {anon, no-grants, one-dir, multi-dir} listing &
      download matrix (`server/tests/api.rs`) — plus S8's expired/revoked/
      ungrant/404/traversal cases, which were cheap once the harness existed

The server side of the protocol is now feature-complete for M1-M3: the
installer can be pointed at it. What remains server-side is S8's deploy
half (Caddyfile + systemd/Docker) and the real packages' sidecars, both in
M7.

## M4 — Installer skeleton (Windows)
- [x] installer/TASKS.md → I1 (toolchain, resources, pinned deps, size gate)
- [x] installer/TASKS.md → I2 + I2a (full UI per `installer/DESIGN.md`)
- [ ] installer/TASKS.md → I3 (WinHTTP client) — NOT started
- [x] Win32 dialog opens; release gate passes
      (script is now `installer/tests/Test-BinarySize.ps1`)
- [ ] WinHTTP GET works against server `GET /packages` (localhost, http)
      — blocked on I3, and on the server reaching M2

NOTE: `installer/DESIGN.md` arrived after this list was written and changed the
installer's shape: it targets THREE games (Richard Burns Rally, Dirt Rally,
BeamNG) behind a game ribbon, not RBR alone. The per-game folder field is
validated against that game's marker executable. See `installer/TASKS.md` I2a.
All installer checks live in `installer/tests/`; run them with
`pwsh -File installer/tests/Run-AllTests.ps1`.

## M5 — Installer cache + install engine
- [ ] installer/TASKS.md → I4..I7
- [ ] Download tab: per-category lists, download → per-user cache
      (sha-keyed, verified), cached/not-cached state shown
      (the old "Library view" wording predates `installer/DESIGN.md`; the UI
      is now a game ribbon with per-category lists, and the lists stay empty
      until this milestone fills them from the manifest)
- [ ] Install engine: ledger (installed.json per game dir), overlay extract,
      one-time backup of non-ledger files, uninstall from ledger
- [ ] Apply = diff selected-vs-ledger; exclusive groups honored;
      `requires` enforced
- [ ] Port `write_rbr_config` (the one built-in special step)

## M6 — Installer auth + polish
- [ ] installer/TASKS.md → I8..I10
- [ ] Access-key entry UI → granted directories appear; localization;
      restore-from-backup
- [ ] Final size audit < 500 kB, `cargo bloat` report saved to docs/

## M7 — Ship
- [ ] Server: Dockerfile or systemd unit + Caddyfile (auto-HTTPS)
- [ ] Sidecar .toml files authored for the real packages (categories,
      display names, exclusive_group on the config variants, requires on
      voice packs)
- [ ] Reproducible release build instructions for both binaries
- [ ] Smoke test: fresh Windows VM — download several packages, install
      subset, switch config variant, uninstall, restore
