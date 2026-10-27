# Development: environment, build, test, release

Everything a person needs to get this repository building and to know that
what they changed still works. For *what* to build next see `TASKS.md`; for
the rules a change has to respect see the `CLAUDE.md` files.

## 1. How the repository is split

| Crate | Builds on | Why |
|---|---|---|
| `shared/` | any OS | Protocol types only (serde). No I/O. |
| `server/` | any OS (deployed on Linux) | axum + tokio + libSQL. Normal async stack. |
| `installer/` | **Windows, MSVC target only** | Win32 GUI + WinHTTP, under a hard 600 kB gate. |

`installer/` is **deliberately excluded** from the Cargo workspace, so a
plain `cargo build` at the root builds `shared` + `server` and nothing else.
That is why the installer has its own CI job, its own lint run and its own
test runner. Do not "fix" the exclusion.

Practical consequence: two `target/` directories, and two places to run
`cargo`. The root, and `installer/`.

## 2. Prerequisites

### 2.1 Windows (installer work)

1. **Rust** — <https://rustup.rs> → `rustup-init.exe`, accept the defaults
   (`stable-x86_64-pc-windows-msvc`).
2. **MSVC build tools** — rustup offers to install them; otherwise
   *Visual Studio Build Tools* with the "Desktop development with C++"
   workload. Needed for `link.exe` and for the resource compiler that
   `embed-resource` drives over `res/installer.rc`.
3. **PowerShell 7 (`pwsh`)** — every script under `installer/tests/` is run
   as `pwsh -File …` and they invoke each other the same way. Windows
   PowerShell 5.1 is not enough.
4. **A desktop session** — the GUI tests drive real dialogs. They cannot run
   over a service account, a container, or an SSH session with no desktop.
5. **`installer/.env`, before the first build** — the server the installer
   talks to by default comes from this file, which git ignores so that no
   deployment's address is ever committed. Without it `cargo build` and
   `cargo test` both fail.

       cd installer
       Copy-Item .env.example .env       # http://127.0.0.1:8080, a local server

   For a release build, set `PACENOTES_DEFAULT_SERVER` in that file to the
   public HTTPS address. The release workflows write it from the repository
   variable of the same name, and fail when that variable is unset.
   `--server <url>` still overrides it at run time.

The Windows box also builds `pacenotes-server`: two of the installer tests
start a real server to talk to. Nothing extra to install for that — but it
means a server-side dependency that breaks on Windows breaks the installer
suite too (this has happened; see §8).

-theirs
```sh
sudo apt update && sudo apt install -y build-essential pkg-config git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # defaults
source "$HOME/.cargo/env"
```

### 2.3 Editor

`rust-lang.rust-analyzer`, plus `ms-vscode-remote.remote-ssh` if you work on
both machines. Open the repo **locally on Windows** for installer work and
**over Remote-SSH on Linux** for server work; keep them in sync through git.
Editing one checkout over SMB/SSHFS is miserable — rust-analyzer and
`target/` over a network mount are slow enough to change how you work.

## 3. Building

```sh
# Linux or Windows — shared + server
cargo build
cargo test

# Windows only — the installer
cd installer
cargo build --target x86_64-pc-windows-msvc            # debug
.\tests\Test-BinarySize.ps1                            # release + size gate
```

`Test-BinarySize.ps1` is the normal way to make a release build: it builds
and then fails if the `.exe` is over 600 kB. **Check the size after every
installer change** — the budget is the reason the installer has no HTTP
crate, no TLS crate and no GUI framework.

## 4. Testing

### 4.1 The installer: one entry point

```powershell
cd installer
pwsh -File tests\Run-AllTests.ps1            # everything
pwsh -File tests\Run-AllTests.ps1 -SkipUi    # headless: steps 1-6 only
```

Eight steps, in order, and what each one is actually for:

| # | Step | What it proves |
|---|---|---|
| 1 | `cargo fmt --check` | The installer is outside the workspace, so the root CI job never formats it. |
| 2 | `cargo clippy` | Same, under `-D warnings` exactly as CI does. |
| 3 | `cargo test` | The pure modules: game detection, the two scene models, sha256, the cache, the run log, size formatting. No Windows needed; CI runs these. |
| 4 | `Test-VersionSync.ps1` | `Cargo.toml` version vs the four VERSIONINFO fields in `res/installer.rc`. The CD workflows tag releases from `Cargo.toml`, so drift ships an `.exe` whose properties contradict its tag. |
| 5 | `Test-BinarySize.ps1` | The 600 kB release gate. Also produces the `.exe` the next three steps run. |
| 6 | `Test-HttpClient.ps1` | Builds and starts a real `pacenotes-server`, then drives the installer's `--selftest-http` against it: health, anonymous vs bearer, 401/403/404, unreachable. WinHTTP cannot be unit tested, so this is the only honest check of it. Needs no desktop, so **CI runs it**. |
| 7 | `ui\Test-UiLayout.ps1` | Walks scenes 1-3 against a deliberately dead server (`--server http://127.0.0.1:1`), once per shipped language, asserting every caption against what `res/installer.rc` defines. |
| 8 | `ui\Test-WizardDownload.ps1` | Walks the **whole wizard**, scenes 1-7, against a live server holding a nested catalogue, with real downloads into a throwaway cache. Asserts both gates, the lists, the summary and the cache on disk. |

Steps 7 and 8 need an interactive desktop. CI skips them; run them locally
before anything that touches the UI.

Two more scripts, neither of them gates:

- `tests\Test-PublicEndpoint.ps1` — **MANUAL CHECK** against the real
  deployment over HTTPS. It is the only test that takes the
  `WINHTTP_FLAG_SECURE` branch and exercises the OS TLS stack, which is the
  justification for shipping no TLS crate. Run it when the deployment
  changes. A red result often says something about the server, not the code.
- `tests\ui\Get-UiScreenshot.ps1` — **AGENT / DEVELOPER AID**. Captures a PNG
  of each scene so a person (or an agent) can *look* at it. Asserts nothing
  and always exits 0. Assertions cannot see a clipped label, two overlapping
  controls or a dead area; a z-order bug once survived a fully green run and
  was caught only here. Use it whenever you change geometry or add a scene.

  ```powershell
  # scenes 1-5 offline, English then Polish
  pwsh -File tests\ui\Get-UiScreenshot.ps1
  pwsh -File tests\ui\Get-UiScreenshot.ps1 -Language 1

  # scenes 6-7 need a real catalogue AND really download it
  pwsh -File tests\ui\Get-UiScreenshot.ps1 -Scenes 7 `
       -Server https://[Replace with IP or domain] -CacheDir $env:TEMP\shotcache
  ```

**Before you add a test, read `installer/tests/README.md`.** It carries two
rules (no arbitrary inputs; expected text comes from the resource, not from a
copy) and a numbered list of traps in driving Win32 from PowerShell — each
one cost real debugging time, and each is commented at its call site.

### 4.2 The server and shared

```sh
cargo test                       # unit tests + server/tests/api.rs
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
```

`server/tests/api.rs` is the auth matrix: anonymous, a credential with no
grants, one directory, several; plus expired, revoked, ungranted, 404 and
traversal attempts.

## 5. Running things by hand

### 5.1 A local server

```sh
mkdir -p data/rbr/packages/public/style data/motd
cp something.zip data/rbr/packages/public/style/Descriptive.zip
echo "hello" > data/motd/en.txt

cargo run -p pacenotes-server -- manifest gen --data ./data
cargo run -p pacenotes-server -- serve --listen 127.0.0.1:8080 --data ./data
curl http://127.0.0.1:8080/health          # -> ok
```

`--db-url` is independent of `--data`: it defaults to `file:./data/auth.db`,
relative to wherever you ran the command, so a `serve` started from the
repository root drops an `auth.db` there whatever `--data` says. `/data/` is
gitignored for that reason — never commit one, it is a credential store.

The folder tree **is** the installer's UI tree — `public/` is an access
directory and a level of its own, and the segments below it become category,
sub-levels and leaf. `manifest gen` prints one line per package and then its
complaints (mis-filed levels, a name that needs percent-encoding, a
dependency that does not exist). Read them. The full layout, access keys and
MOTD are in **`deploy/README.md`**.

Credentials, when you need a private directory:

```sh
cargo run -p pacenotes-server -- token mint --label "dev" --grant premium
cargo run -p pacenotes-server -- token list
```

A key is printed **once**; the server stores only its sha256. The credential
store defaults to `./data/auth.db`, and `--db-url` belongs to `token`, before
the subcommand (`token --db-url file:… mint …`) — putting it after `mint`
is a usage error.

### 5.2 The installer against that server

```powershell
cd installer
$exe = "target\x86_64-pc-windows-msvc\release\pacenotes-installer.exe"

& $exe --server http://127.0.0.1:8080 --cache $env:TEMP\pcn-cache
```

| Flag | For |
|---|---|
| `--server <url>` | Point at a local or staging server. Default is the public HTTPS endpoint. |
| `--cache <dir>` | A throwaway download cache. Use it for anything test-shaped, or you read back whatever a previous run left in your own profile. |
| `--selftest-http <url> [--token T] [--out FILE]` | One real GET through `http.rs`, reported to a file or a message box. |
| `--selftest-download <game> [--largest] [--server U] [--token T] [--out F]` | Fetch a catalogue and download its smallest (or largest) package: stream, hash, verify, promote, index. |
| `--selftest-report <game> [--server U] [--out F]` | POST a fabricated conflict report through `api::report_conflicts`. The only thing the installer ever sends, and a real one needs a catalogue where two packages write the same file. |
| `--selftest-panic [--out F]` | Panic on purpose, with the crash window suppressed, to prove the panic hook writes a report naming a source file and line. |

The four `--selftest-*` flags exist because the paths they cover are the ones
most likely to be wrong against a real server, and walking the GUI to reach
them makes every attempt slow and flaky. Plain HTTP is fine for development;
WinHTTP only negotiates TLS when the URL says `https`.

Where a run leaves things, when `--cache` is not given:

```text
<game dir>\.pacenotes\cache\<sha256>.zip        verified payloads
<game dir>\.pacenotes\cache\cache.json          what each one is
<game dir>\.pacenotes\logs\install-<stamp>.log  one log per run
<game dir>\.pacenotes\installed.json            the ledger
<game dir>\.pacenotes\backup\, restore\<stamp>\ what was replaced
```

Everything, under the folder the installer was pointed at. Deleting the
game folder removes every trace of the tool. `--cache <dir>` overrides the
cache location for development; nothing else can be moved, and nothing is
written to `%LOCALAPPDATA%` or anywhere else in the profile.

## 6. Releasing

The installer version lives in **two** places and they must agree —
`Test-VersionSync.ps1` enforces it, and CI runs that check:

1. `installer/Cargo.toml` → `[package] version`
2. `installer/res/installer.rc` → `FILEVERSION`, `PRODUCTVERSION`, and the
   `FileVersion` / `ProductVersion` strings

Then merge a PR into `master`, or run a workflow by hand. A plain push to
`master` runs CI but publishes nothing.

| Workflow | Trigger | Result |
|---|---|---|
| `ci.yml` | push / PR to `master` | shared+server on Linux, installer on Windows (fmt, clippy, tests, version sync, size gate, HTTP client) |
| `cd-deploy-on-pull-request.yml` | a PR into `master` is **merged** | Release build, tags `v<version>`, publishes the `.exe` as a GitHub release |
| `cd-deploy-manually.yml` | `workflow_dispatch` | The same, on demand |
| `cd-debugbuild-manually.yml` | `workflow_dispatch` | Debug `.exe` + `.pdb` as a build artifact, no release |

The two deploy workflows **stop** if the tag for that version already
exists, so bumping the version *is* the release step. (The debug-build one
only warns — it publishes nothing to tag.) Tags use a
`v/x.y.z` shape.

Deploying the **server** is a separate thing and lives in
`deploy/README.md`: a Podman Quadlet that builds straight from git, plus a
Caddy site block for TLS.

## 7. Conventions

- **Follow the task lists.** `TASKS.md` (root milestones) → `installer/TASKS.md`
  and `server/TASKS.md`. Tick items as you go.
- **One commit per task-list item**, not per file touched. Say in the message
  what changed and *why*, including what you decided not to do.
- **Record the installer's size** in `installer/TASKS.md` after a change that
  moves it; the table there is the history of the budget.
- A protocol change is `shared/` first, then both sides, in the same PR.
- Prefer std over a dependency everywhere, and especially in `installer/`.
- Read the `CLAUDE.md` in the directory you are about to change. They carry
  the rules that are not obvious from the code — the size budget, the
  "installer never interprets package content" rule, the auth model, and the
  reason the folder layout on the server is the UI tree.

## 8. Things that will bite you

**The installer suite builds the server, so a Linux-only dependency breaks
Windows.** `libsql` once lost the feature that transitively enabled
`libsql-sys/rusqlite`, and without it `libsql-sys` compiles
`std::os::unix::ffi::OsStrExt` — on Windows. `server/Cargo.toml` now declares
`libsql-sys` with `rusqlite` explicitly to pin that down.

**`RUSTFLAGS` leaks between crates.** `Run-AllTests.ps1` sets `-D warnings`
to match CI, and saves/restores it around the run — a stray value forces a
full rebuild of the server's dependency tree and applies the installer's lint
policy to a crate with a different one.

**`cargo test` at the root does not test the installer**, and
`cargo clippy --workspace` does not lint it. That is what steps 1-3 of the
runner are for.

**The GUI tests are not optional for UI work.** They are the only thing that
catches a caption bound to the wrong id, a gate that does not gate, or a
control that never got its text. But they are *also* not sufficient: run
`Get-UiScreenshot.ps1` and actually look at the result.
