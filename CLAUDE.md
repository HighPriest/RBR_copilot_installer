# Pacenotes — project overview (root CLAUDE.md)

Rust rewrite of a C#/WinForms + Supabase installer for RBR (Richard Burns Rally)
pacenote packages. Two deliverables:

1. **`installer/`** — Windows-only GUI installer. HARD constraint: final
   `.exe` < 700 kB. See `installer/CLAUDE.md` before touching it.
   (Was 500 kB until 2026-10-25, when the install engine landed, and 600 kB
   until 2026-10-31, when it stopped needing the Visual C++ redistributable,
   and 620 kB until 2026-11-01, when the build reached it exactly and the
   maintainer raised it. The rules it produced did not move.)
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
- **Nothing is written outside the game folder.** Every trace of this
  installer — cache, ledger, backups, restore points, run logs — lives in
  that folder's `.pacenotes\`. Deleting the game removes all of it, and a
  folder copied to another drive or machine arrives complete.
- **Cache**: downloads land in `<game dir>\.pacenotes\cache\<sha256>.zip`
  plus a `cache.json` index. Sha-keyed: integrity, dedupe, and an older
  version stays reachable after the index moves on — which is what lets a
  restore roll an update back. Installing is cache → game dir, so it works
  offline. Two game folders no longer share one cache: that is the price of
  the rule above, and it is paid deliberately.
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
    data/rbr/schema.toml      the game's install schema (no schema = the
                              game publishes nothing)
    data/motd/<lang>.txt      message of the day (no game: shown before one
                              is chosen)
    data/reports/             conflict reports from installers
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
  the caller's granted directories (silent filtering; /packages never 4xxs —
  a bad, expired or revoked key simply gets the public listing, and a DB
  outage does too). Without `game` the document covers EVERY game; with it,
  just that one. An unknown game is an empty list, not an error.
- `GET /download/{game}/{directory}/{*name}` — `name` spans the remaining
  segments, because it is the package's path under its directory
  (`PackageInfo::download_path()` builds the whole tail). Same auth rules;
  streams `./data/<game>/packages/<directory>/<name>.zip`. The status never
  reveals what a protected directory holds: no live key on a non-public
  directory → 401 WHETHER OR NOT the package exists; a live key without that
  grant, or no such package → 404. There is no 403 (it said "exists, but not
  for you"). A name may contain characters that need percent-encoding (e.g.
  `style/Numeric Standard`); the client must encode.
- `GET /whoami` — `Authorization: Bearer <token>`. 200
  `shared::KeyStatus { expires_at }` for a live key, 401 for anything else
  (none, malformed, unknown, expired, revoked). This — not `/packages` — is
  how a client validates a key.
- `GET /motd?lang=xx` — `text/plain` from `data/motd/<lang>.txt`, falling
  back to `en`, 404 when there is none (treat exactly like being offline).
  No auth. `lang` is restricted to `[a-z]{2,8}` server-side.
- `GET /health` — 200, no auth.
- `POST /report/conflict` — no auth, body `shared::ConflictReport`
  `{game, installer, conflicts: [{file, packages: ["dir/name", …]}]}`, the
  packages of each file in INSTALL ORDER (last = winner). An installer
  reports that selected packages wrote the same file. It does NOT block the
  user: the later package wins and the operator, who is the only one who
  can fix such a catalogue, gets told. Carries no user identity and no local
  paths; the server stamps the time and stores no IP. Unauthenticated
  write, so: 8 KiB cap (413), validated fields with bad entries dropped
  (400 if none survive), rate-limited per client + globally (429 with
  `Retry-After`; the client is taken from `X-Forwarded-For` only when the
  peer is a `--trusted-proxy`), append-only JSONL, empty 202 on success.
  See `server/TASKS.md` S10.
- The manifest carries each game's INSTALL SCHEMA (`Manifest.schemas`,
  `shared::GameSchema`): the game's categories, which ask for one pick, and
  where each installs — plus the SHA-256 of the schema file's RAW BYTES. It
  replaces the earlier "manifest version": what an installer can fall behind
  on is the shape of a game's catalogue, not the package list. The installer
  uses it as a COMPATIBILITY CHECK ONLY (its behaviour and localized labels
  stay compiled in; `install_root` is a token, never a path, so a server can
  never steer where files go). A hash the installer does not know blocks
  DOWNLOADING for that game only — reconfiguring and uninstalling keep
  working. The canonical files are `schemas/<game>.toml` (byte-exact,
  `.gitattributes` `-text`); installers embed them. See `server/TASKS.md`
  S11 and `deploy/README.md` for the rollout order.

Any protocol change = change `shared/` first, then both sides in the same PR.

## Reference material

`legacy/` contains the original C# sources (`DownloadManager.cs`,
`InstallationForm.cs`, `Program.cs`). Port *behavior users depend on*
(overlay layout, ini generation, restore-to-pristine), not mechanisms the
new install model supersedes (backup/FilipekMod/unpacked staging dirs,
oldest-zip restore). `installer/CLAUDE.md` says which is which.

## Documentation

- `docs/` is an mdBook that CI publishes to GitHub Pages
  (`.github/workflows/docs.yml`); `docs/build.sh` builds the same site
  locally. See `DEVELOPMENT.md` §3.1.
- A document lives where it belongs in the repository. Its book chapter is
  one `include` line at the same relative path under `docs/src/`, so links
  between documents work in both places; add that line and a
  `docs/src/SUMMARY.md` entry for a new document. Links to source files are
  pointed at GitHub by `tools/mdbook-repo-links`.
- Diagrams are D2 sources in `docs/diagrams/`, rendered to SVG at build
  time; the SVGs are never committed.
- The API reference is rustdoc with private items. The installer's is
  built with `PACENOTES_ENV_FILE=.env.example`: rustdoc prints the default
  server, and a published page must never carry a real deployment's address.
- **No Python anywhere in the project**, documentation tooling included.

## Workspace layout / build

- Root `cargo build` builds `shared`, `server` and `tools/mdbook-repo-links`
  (Linux OK, used in CI).
- A new workspace member must also be `COPY`'d in `deploy/Containerfile`:
  cargo reads every member's manifest even to build only the server. CI
  loads the workspace from exactly what that file copies; a release found
  the gap first, at v/2.0.1.
- `installer/` builds only on Windows MSVC target and is excluded from the
  workspace default members on purpose. Do not "fix" that.
- Check binary size after every installer change:
  Windows: `installer/tests/Test-BinarySize.ps1` (fails > 700 kB, or on a
  DLL Windows does not ship).

## Working conventions for Claude Code sessions

- Follow the task lists: `TASKS.md` (root, ordered milestones), then
  `installer/TASKS.md` and `server/TASKS.md`. Mark items done as you go,
  and record work you discover mid-task as a SUB-POINT of that task.
- `requirements.md` sits beside each of those lists and says what a task
  must satisfy, including which verification applies. `.claude/session.md`
  says what is being worked on right now. Task lists carry no prose: a
  rule belongs in a `CLAUDE.md`, behaviour in `DESIGN.md`, the story of a
  change in `CHANGELOG.md`.
- Prefer std over dependencies everywhere, but especially in `installer/`.
- Limit amount of `use` statements. Prefer the full path at the point of use
  (`std::path::PathBuf`, `pacenotes_shared::Manifest`, `anyhow::bail!(...)`),
  most importantly in struct/enum definitions and function signatures. Where a trait method is needed, call it fully qualified
  (`<sha2::Sha256 as sha2::Digest>::new()`). This keeps every identifier's
  origin visible at the call site.
- Windows paths in install logic use `\\` separators intentionally (RBR
  expects them in .ini content). Do not "normalize" .ini file contents.
- No unwrap() in server request paths; map errors to proper HTTP statuses.
- Installer: no unwrap()/expect()/panic! outside tests — clippy denies them
  (see `installer/CLAUDE.md`, "Error handling"); a failure shows a
  MessageBox and the installer recovers or exits gracefully.
