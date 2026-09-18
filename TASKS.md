# Project task list (ordered milestones)

Work top-to-bottom. Each milestone leaves the repo in a buildable state.
Detailed per-crate steps live in `installer/TASKS.md` and `server/TASKS.md`.

## M0 — Repo bootstrap
- [x] Workspace layout (`shared/`, `installer/`, `server/`)
- [x] CLAUDE.md files (root, installer, server)
- [x] Copy legacy C# sources into `legacy/` for reference
- [x] `git init`, first commit
- [ ] Pin all `*`/loose versions in Cargo.toml files to exact versions
      (`cargo add` latest stable of each; commit Cargo.lock)
- [ ] Setup github actions scripts, equivalent to `legacy\.github\workflows`

## M1 — Shared protocol crate
- [ ] `shared`: PackageInfo (directory-based), Manifest, ApiError compile
- [ ] Unit tests: JSON round-trip; Manifest::visible_to grant filtering
- [ ] `cargo test -p pacenotes-shared` green

## M2 — Server MVP (serves real files, no auth yet)
- [ ] server/TASKS.md → S1..S4
- [ ] `GET /health`, `GET /packages` (public only), `GET /download/{name}`
- [ ] Manual test with curl against a `packages/` dir with one test zip

## M3 — Server auth
- [ ] server/TASKS.md → S5..S7
- [ ] libSQL schema + migrations (tokens + grants), token hashing,
      directory-grant checks (auth::decide)
- [ ] Admin CLI subcommand: mint/revoke credentials
- [ ] Admin CLI: grant/ungrant directories per credential
- [ ] Integration tests: {anon, no-grants, one-dir, multi-dir} listing &
      download matrix

## M4 — Installer skeleton (Windows)
- [ ] installer/TASKS.md → I1..I3
- [ ] Win32 dialog opens, size-check script passes on empty-ish binary
- [ ] WinHTTP GET works against server `GET /packages` (localhost, http)

## M5 — Installer download + install pipeline
- [ ] installer/TASKS.md → I4..I7
- [ ] List packages in UI, download w/ progress, sha256 verify, unzip
- [ ] Port RBR logic: backup, unpack, install, ini generation

## M6 — Installer auth + polish
- [ ] installer/TASKS.md → I8..I10
- [ ] Access-key entry UI → granted directories appear; localization;
      uninstall/restore
- [ ] Final size audit < 500 kB, `cargo bloat` report saved to docs/

## M7 — Ship
- [ ] Server: Dockerfile or systemd unit + Caddyfile (auto-HTTPS)
- [ ] Reproducible release build instructions for both binaries
- [ ] Smoke test: fresh Windows VM installs a real package end-to-end
