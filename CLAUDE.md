# Pacenotes — project overview (root CLAUDE.md)

Rust rewrite of a C#/WinForms + Supabase installer for RBR (Richard Burns Rally)
pacenote packages. Two deliverables:

1. **`installer/`** — Windows-only GUI installer. HARD constraint: final
   `.exe` < 600 kB. See `installer/CLAUDE.md` before touching it.
   (Was 500 kB until 2026-10-25, when the install engine landed. The
   number moved once, deliberately; the rules it produced did not.)
2. **`server/`** — package/auth server. Single static binary, libSQL (SQLite)
   for auth, packages stored as flat files on disk. See `server/CLAUDE.md`.
3. **`shared/`** — protocol types ONLY (serde structs). No I/O, no async, no
   HTTP. Both sides depend on it; nothing else is shared.

## Non-negotiable architecture rules

- The installer must NOT pull in tokio, reqwest, rustls, native-tls, openssl,
  hyper, or any GUI framework crate. HTTPS goes through **WinHTTP** (OS
  Schannel). This is what keeps the binary small, and it stays true
  whatever the gate's number is — the forbidden crates are forbidden
  because of what they drag in, not because of 100 kB of slack.
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
- **Where a package SITS on the server is how it is shown.** `PackageInfo.name`
  is the package's path under its access directory, without the `.zip`:

      packages/public/codriver/polish/male/Kamil.zip
               ^dir   ^------------- name -------^

  Its segments ARE the UI tree: first = `category`, middle = tree levels,
  last = the package. `shared` derives all three (`category()`,
  `tree_levels()`, `leaf()`); neither side stores them separately. Organising
  the files on disk organises the installer's Download tree, and sidecars
  carry no positional metadata at all.
  - Access `directory` is auth ONLY and is a separate level of the layout, so
    the same category can appear under `public/` and a private directory.
  - `name` is a LOOKUP KEY, never joined onto a filesystem path. Validate it
    with `shared::valid_name` on both sides: no `\`, no `.`/`..` segment, no
    empty segment. See the security invariants in `server/CLAUDE.md`.
  - Deriving the tree from storage keeps one source of truth and makes the
    operator's own folders meaningful. It does NOT weaken the rule that the
    installer never interprets package CONTENT: nothing inside a zip is read
    for meaning, and a compromised server still cannot gain arbitrary
    installer behavior.
- The rest stays declarative metadata in the manifest, never files in the zip:
  - `display_name` (optional): renames the LEAF only; the levels above it come
    from `name`.
  - `exclusive_group` (optional): packages sharing a group overwrite the
    same files (e.g. the pacenote-config variants); the UI radio-selects
    within a group and Apply uninstalls the previous member first.
  - `requires` (optional): list of "directory/name" deps the UI enforces
    (e.g. voice pack requires its language pack). The name half may itself
    contain `/`, so a ref splits on the FIRST separator only.
- **Cache**: downloads land in a per-user cache
  (`%LOCALAPPDATA%\PacenotesInstaller\cache\<sha256>.zip` + `cache.json`
  index). Sha-keyed: integrity + dedupe. Installing is cache → game dir;
  works offline, works for multiple game dirs.
- **Ledger**: each game dir gets `.pacenotes\installed.json` recording, per
  installed package, the exact files written. Uninstall = delete ledgered
  files. Apply = diff (selected vs ledger) → uninstall removed, install
  added. The location is the SAME for every game: a path inside the plugin
  tree would be RBR-only, and the plugin tree is written by other tools.
- **Backup**: before overwriting a file NOT owned by the ledger, copy it
  once to `.pacenotes\backup\` (preserving relative path). Restore = copy
  back. A file whose bytes already match what is about to be written is NOT
  copied: there is nothing to preserve, and a blind copy would duplicate
  hundreds of MB of identical content on the first run.
  This replaces the legacy backup/FilipekMod/unpacked directory dance.
- **Per-game configuration step**: after the install phase, each game may
  run a step of ITS OWN. There is no shared abstraction and no attempt to
  make one: the games do not share a file layout, several do not have a
  plugin system at all, and their installers may have to edit game files
  directly. RBR's step modifies the existing Pacenote config (or writes one
  from a built-in template when there is none); another game's step will
  look nothing like it. What is shared is the plumbing — the step reports
  the files it wrote, and they are ledgered and backed up like any other.

## Server layout (one server, every game)

ONE server serves all games. The first level under its data root names the
game, and each game owns its own packages tree:

    data/rbr/packages/public/codriver/polish/male/Kamil.zip
         ^game         ^dir   ^------------ name -----^
    data/motd/<lang>.txt      message of the day (no game: shown before one
                              is chosen)
    data/manifest.json        every game in one document

`game` therefore comes from the LAYOUT, never from a sidecar — the same rule
as `category`/tree position. Game ids and access directory names share one
rule, `shared::valid_id` (`[a-z0-9_-]+`). `manifest gen` warns when a level
is mis-filed (a category where an access directory belongs, an access
directory or category where a game belongs, a game folder with no
`packages/`) and when a name needs percent-encoding.

## Protocol (client ↔ server)

- `GET /packages[?game=<id>]` — optional `Authorization: Bearer <token>`.
  Returns `shared::Manifest` (JSON) filtered to the `public` directory plus
  the caller's granted directories (silent filtering; /packages never 403s).
  Without `game` the document covers EVERY game; with it, just that one. An
  unknown game is an empty list, not an error.
- `GET /download/{game}/{directory}/{*name}` — `name` spans the remaining
  segments, because it is the package's path under its directory
  (`PackageInfo::download_path()` builds the whole tail). Same auth rules;
  streams `./data/<game>/packages/<directory>/<name>.zip`. 401 bad/expired
  token on a protected directory, 403 valid token without that grant, 404 no
  such package — and a miss on ANY of the three keys is a 404, so the status
  never reveals what exists. A name may contain characters that need
  percent-encoding (e.g. `style/Numeric Standard`); the client must encode.
- `GET /motd?lang=xx` — `text/plain` from `data/motd/<lang>.txt`, falling
  back to `en`, 404 when there is none (treat exactly like being offline).
  No auth. `lang` is restricted to `[a-z]{2,8}` server-side.
- `GET /health` — 200, no auth.
- `POST /report/conflict` — no auth, JSON. An installer reports that two
  selected packages wrote the same file. It does NOT block the user: the
  later package wins and the operator, who is the only one who can fix
  such a catalogue, gets told. Carries no user identity and no local
  paths — package refs and relative paths are catalogue data the server
  already has. Unauthenticated write, so: capped body, validated fields,
  rate-limited, append-only storage. See `server/TASKS.md` S10.
- The manifest carries a VERSION. Choice categories live in the installer
  (their labels and explanations are localized UI text, which cannot come
  from the server), so a catalogue can outgrow an old build. An installer
  that sees a newer manifest blocks DOWNLOADING only — reconfiguring and
  uninstalling what the user already has keeps working. See `server/TASKS.md`
  S11.

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
  Windows: `installer/tests/Test-BinarySize.ps1` (fails > 600 kB).

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
