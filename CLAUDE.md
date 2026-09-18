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

`legacy/` (if present) contains the original C# sources (`DownloadManager.cs`,
`InstallationForm.cs`, `Program.cs`). The installer's install/backup/uninstall
logic (RBR .ini generation, backup zip, unpack/copy/delete flows) must be
ported from there behavior-for-behavior. When in doubt about behavior, read
the C# and match it — including quirks like "oldest backup zip is the restore
source".

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
- Windows paths in install logic use `\\` separators intentionally (RBR
  expects them in .ini content). Do not "normalize" .ini file contents.
- No unwrap() in server request paths; map errors to proper HTTP statuses.
- Installer: unwrap()/panic is acceptable only during early scaffolding; final
  code shows a MessageBox and exits gracefully.
