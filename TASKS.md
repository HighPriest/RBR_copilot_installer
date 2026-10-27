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
- [x] installer/TASKS.md → I3 (WinHTTP client, `--selftest-http`)
- [x] Win32 dialog opens; release gate passes
      (script is now `installer/tests/Test-BinarySize.ps1`)
- [x] WinHTTP GET works against server `GET /packages` (localhost, http)
      — verified end to end against a real `pacenotes-server`, including
      bearer auth and the 401/404/unreachable surfaces:
      `installer/tests/Test-HttpClient.ps1`, 28/28. It needs no desktop, so
      CI runs it.

M4 IS COMPLETE. The installer opens, is localized, validates a game folder per
game, and can talk to the server. It cannot yet DO anything with what it
fetches — parsing the manifest into the ribbon's lists is I5 (M5).

NOTE: `installer/DESIGN.md` arrived after this list was written and changed
the installer's shape twice. It targets FOUR games (Richard Burns Rally,
Dirt Rally 2.0, Assetto Corsa, BeamNG), and rev. 2 replaced the game-ribbon
layout with a SEVEN-SCENE WIZARD — one game per run, chosen on scene 3 and
validated against that game's marker executable. Anywhere below that says
"ribbon", "tab" or "Library view" predates that and means a wizard scene.
See `installer/TASKS.md` I2a/I2b. All installer checks live in
`installer/tests/`; run them with
`pwsh -File installer/tests/Run-AllTests.ps1`.

## M5 — Installer cache + install engine
- [x] installer/TASKS.md → I4 (worker + progress/cancel protocol)
- [x] installer/TASKS.md → I5 (api, std-only sha256, the sha-keyed cache,
      verified downloads)
- [x] Download scene: one tree built from where packages SIT on the server,
      download → per-user cache (sha-keyed, verified), per-leaf state shown
      (not cached / downloaded / update / local only)
- [x] Scenes 5-7: the three configuration picks, a progress scene that
      really downloads, and a summary
- [x] installer/TASKS.md → I6 (zip reader)
- [x] Install engine (I7): ledger (installed.json per game dir), overlay
      extract, one-time backup of non-ledger files, uninstall from ledger
- [x] Apply = diff selected-vs-ledger; exclusive groups honored;
      `requires` enforced
- [x] Per-game configuration step, each game owning its own (no shared
      abstraction — the games share no file layout and some have no plugin
      system). RBR's edits the pacenotes language and voice into the
      Pacenote plugin's config, preserving the rest; it is NOT a port of
      `write_rbr_config`, whose output the packages have outgrown. See
      `installer/TASKS.md` I7 and `installer/DESIGN.md`.
- [x] NOT in the original plan, decided on 2026-10-26: downloading and
      installing are two SCREENS with the configuration step between them,
      and scene 3 offers the two routes that implies (Download/Update and
      Configure/Install). A download fills a cache; an install writes to
      the game folder; a user should be able to do either without the
      other. See `installer/DESIGN.md` "Flow".

M5 IS DONE. The wizard runs end to end, writes to the game folder, records
what it wrote, backs up what it replaced and takes a restore point. What
the engine CANNOT yet do is use those restore points — that is the restore
scene, which M6 carries.

## M6 — Installer auth + polish

- [x] installer/TASKS.md → I8: the access key is the bearer for every
      request, and it is validated with `GET /whoami` (the only endpoint
      that can: `/packages` answers the public listing for a key it does
      not accept). A key near expiry says so, with the date
- [ ] installer/TASKS.md → I9 (polish) and I10 (release gate)
- [x] **Restore scene** (2026-10-27). A user handed a bad package can go
      back to any earlier state, or remove everything this installer
      added, after seeing exactly what that would do. See
      `installer/TASKS.md` "Restore" and `installer/DESIGN.md`
- [ ] Final size audit < 600 kB. `docs/bloat.txt` is saved; currently
      576.5 kB, after the icon went from 85 kB (one 256px frame) to 45 kB
      (16/32/48 BMP + 128 PNG) to pay for the restore scene

## M7 — Ship

- [x] Server: Containerfile + Podman Quadlet + Caddy site block, and
      `deploy/README.md` covering configuration, publishing, access keys,
      MOTD and troubleshooting (server/TASKS.md S8)
- [ ] CI builds the image, tags it with the git sha and pushes it, so the
      Quadlet pins a tag and a restart never builds (DESIGN-REVIEW O1)
  - [ ] Build both server and installer executables in CI.
  - [ ] Store artifacts of the build in CI
  - [ ] Server: During deployment from Quadlet, source the executable from CI
- [ ] Sidecar .toml files authored for the real packages (categories,
      display names, exclusive_group on the config variants, requires on
      voice packs)
- [ ] Reproducible release build instructions for both binaries
- [ ] Smoke test: fresh Windows VM — download several packages, install
      subset, switch config variant, uninstall, restore
