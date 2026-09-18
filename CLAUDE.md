# Pacenotes — project overview (root CLAUDE.md)

Rust rewrite of a C#/WinForms + Supabase installer for RBR (Richard Burns Rally)
pacenote packages. Two deliverables:

1. **`installer/`** — Windows-only GUI installer. HARD constraint: final
   `.exe` < 500 kB. See `installer/CLAUDE.md` before touching it.
2. **`server/`** — package/auth server. Single static binary, libSQL (SQLite)
   for auth, packages stored as flat files on disk. See `server/CLAUDE.md`.
3. **`shared/`** — protocol types ONLY (serde structs). No I/O, no async, no
   HTTP. Both sides depend on it; nothing else is shared.

## Non-negotiable architecture rules

- The installer must NOT pull in tokio, reqwest, rustls, native-tls, openssl,
  hyper, or any GUI framework crate. HTTPS goes through **WinHTTP** (OS
  Schannel). This is what keeps the binary under 500 kB.
- The server is a normal async stack (axum + tokio). Do not try to make the
  server share HTTP code with the installer.
- `shared/` stays dependency-minimal: `serde` only. If you're about to add a
  dependency to `shared`, stop and reconsider.
- Auth model: bearer tokens (credentials). Server stores `sha256(token)`,
  never the raw token. NO TIERS: packages live in named directories
  (`packages/<directory>/<name>.zip`); each credential holds grants, each
  grant naming one directory it may list/download; a credential can hold any
  number of grants. The `public` directory is visible to everyone. A grant
  covers its whole directory — per-package access = a package in its own
  directory.

## Install model (fixed mechanism, declarative metadata)

- Every package zip is a **game-root overlay**: extracting it at the game
  directory puts every file in its final place. This is the ONLY install
  mechanism. There are NO instruction/script files inside packages and the
  installer must never execute or interpret package content. Rationale:
  a compromised server must not gain arbitrary installer behavior; an
  interpreter costs size budget; all real packages already conform.
- What varies between packages is **metadata**, carried by the server
  manifest (`PackageInfo`), NOT by paths and NOT by files inside the zip:
  - `category`: UI grouping ("sounds", "configs", "languages", "pacenotes";
    open string). Replaces the legacy meaning-in-path scheme
    (`sounds\lang\gender\Voice.zip`). Access `directory` is auth ONLY —
    category and directory are orthogonal.
  - `display_name`: human name shown in the UI (replaces path hierarchy,
    e.g. "English / Female / VoiceName").
  - `exclusive_group` (optional): packages sharing a group overwrite the
    same files (e.g. the pacenote-config variants); the UI radio-selects
    within a group and Apply uninstalls the previous member first.
  - `requires` (optional): list of "directory/name" deps the UI enforces
    (e.g. voice pack requires its language pack).
- **Cache**: downloads land in a per-user cache
  (`%LOCALAPPDATA%\PacenotesInstaller\cache\<sha256>.zip` + `cache.json`
  index). Sha-keyed: integrity + dedupe. Installing is cache → game dir;
  works offline, works for multiple game dirs.
- **Ledger**: each game dir gets `Plugins\Pacenote\installer\installed.json`
  recording, per installed package, the exact files written. Uninstall =
  delete ledgered files. Apply = diff (selected vs ledger) → uninstall
  removed, install added.
- **Backup**: before overwriting a file NOT owned by the ledger, copy it
  once to `backup\` (preserving relative path). Restore = copy back.
  This replaces the legacy backup/FilipekMod/unpacked directory dance.
- The one built-in special step: `write_rbr_config` (RBR ini generation),
  hard-coded in the installer, keyed off the selected `configs` package.

## Protocol (client ↔ server)

- `GET /packages` — optional `Authorization: Bearer <token>`.
  Returns `shared::Manifest` (JSON) filtered to the `public` directory plus
  the caller's granted directories (silent filtering; /packages never 403s).
- `GET /download/{directory}/{name}` — same auth rules; streams
  `./packages/<directory>/<name>.zip`. 401 bad/expired token on a protected
  directory, 403 valid token without that grant, 404 no such package.
- `GET /health` — 200, no auth.

Any protocol change = change `shared/` first, then both sides in the same PR.

## Reference material

`legacy/` contains the original C# sources (`DownloadManager.cs`,
`InstallationForm.cs`, `Program.cs`). Port *behavior users depend on*
(overlay layout, ini generation, restore-to-pristine), not mechanisms the
new install model supersedes (backup/FilipekMod/unpacked staging dirs,
oldest-zip restore). `installer/CLAUDE.md` says which is which.

## Workspace layout / build

- Root `cargo build` builds `shared` + `server` (Linux OK, used in CI).
- `installer/` builds only on Windows MSVC target and is excluded from the
  workspace default members on purpose. Do not "fix" that.
- Check binary size after every installer change:
  Windows: `installer/check-size.ps1` (fails > 500 kB).

## Working conventions for Claude Code sessions

- Follow the task lists: `TASKS.md` (root, ordered milestones), then
  `installer/TASKS.md` and `server/TASKS.md`. Mark items done as you go.
- Prefer std over dependencies everywhere, but especially in `installer/`.
- Limit amount of `use` statements. Prefer the full path at the point of use
  (`std::path::PathBuf`, `pacenotes_shared::Manifest`, `anyhow::bail!(...)`),
  most importantly in struct/enum definitions and function signatures. Where a trait method is needed, call it fully qualified
  (`<sha2::Sha256 as sha2::Digest>::new()`). This keeps every identifier's
  origin visible at the call site.
- Windows paths in install logic use `\\` separators intentionally (RBR
  expects them in .ini content). Do not "normalize" .ini file contents.
- No unwrap() in server request paths; map errors to proper HTTP statuses.
- Installer: unwrap()/panic is acceptable only during early scaffolding; final
  code shows a MessageBox and exits gracefully.
