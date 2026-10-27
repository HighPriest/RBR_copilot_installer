<p align="center">
<h1 align="center"><b>RBR Copilot Installer</b></h1>
<p align="center">Richard Burns Rally<br>
Esports ready Co-pilot<br>
The ultimate second seat filler</p>
<p align="center"><a href="https://youtu.be/fZ3FEQRCJog"><img src="https://i3.ytimg.com/vi/TkRz2F80gec/mqdefault.jpg"/></a><br>
<b align="center">Demo Video</b>
</p>

## Introduction

This repository is the source for an automated installer of CoDriver
configuration files — pacenote packages for Richard Burns Rally and other
rally games. It is presented so you can see the current state of the
project, point out bugs, and say what you want from the finished product.

It is a Rust rewrite of the original C#/WinForms installer, and it comes
with its own package server instead of a third-party backend. Two things
ship from here:

- **The installer** — one small Windows `.exe`. It downloads packages, verifies
  them, and keeps them in a per-user cache so a second run costs nothing.
- **The server** — one binary that serves those packages as flat files from
  a directory tree, with optional access keys for private content. No
  database to provision, no upload API.

---

## Using the installer

Download `pacenotes-installer.exe` from the releases page and run it. It is
one file: no setup wizard, nothing in Program Files, nothing in the
registry.

It does need the **Microsoft Visual C++ 2015-2022 Redistributable (x64)**,
which almost every PC that runs games already has. If Windows complains
about `vcruntime140.dll`, that is the thing to install.

The wizard:

1. **Language** — English or Polish; it starts in your Windows language.
2. **Message of the day** — what the server has to say, plus a field for an
   access key if you have one. Leave it empty for public content. If the
   server cannot be reached you can carry on with whatever you already
   downloaded.
3. **Game folder** — point it at the folder holding the game's own
   executable. It identifies the game from that, and then offers two
   buttons:
   - **Download/Update** — get new content, then set it up and install it.
   - **Configure/Install** — change what is active using what is already on
     this computer. No server needed, so this is the quick way to swap
     co-driver or style later.
4. **Download** — a tree of everything available, built from how the
   packages are filed on the server. Tick what you want on this computer.
   Anything you already have is ticked for you and marked.
5. **Downloading** — progress, the package currently being fetched, and a
   live log. Cancel is available here; an unfinished download is discarded.
   Nothing has been written to the game folder at this point.
6. **Configuration** — choose the one pacenotes language, one co-driver
   voice and one style to actually use. Voices are listed with their full
   path, so two voices with the same name stay distinguishable. What you
   already have installed is pre-selected, so changing one thing does not
   mean answering everything again.
7. **Installation** — the backup and install, with the same progress screen
   and log. This part needs no server.
8. **Summary** — what was done, how much was downloaded, and where the log
   is. The log is hidden when everything went well and shown when it did
   not.

What a run leaves on your machine:

```text
<game folder>\.pacenotes\    everything, and nothing anywhere else:
                             the downloaded packages, what was installed,
                             what was replaced, the restore points, and
                             one log per run
```

**Nothing is written outside the game folder** — not to your profile, not to
the registry, not to Program Files. Deleting the game folder removes every
trace of this installer, and copying that folder to another drive or PC
brings your downloads and your restore points with it.

The downloads are kept and named by content hash, so re-running the
installer re-uses what you have and works offline. Two separate game folders
each keep their own copy; that is the cost of leaving nothing behind.

If you used a version before 2026-10-27, it kept downloads in
`%LOCALAPPDATA%\PacenotesInstaller`. Nothing reads that folder any more and
you can delete it.

---

## Deploying the package server

One Podman Quadlet builds the server straight from this repository, one
Caddy site block puts it on HTTPS, and one directory holds everything
mutable. There is no database to provision and no upload API: **publishing a
package is copying a zip into place.**

```sh
cp deploy/pacenotes.container deploy/pacenotes.config.env \
   ~/.config/containers/systemd/
systemctl --user daemon-reload && systemctl --user start pacenotes
```

**Start with [`deploy/README.md`](deploy/README.md).** It covers the data
layout (which decides what the installer's tree looks like), publishing
packages, optional metadata, the message of the day, access keys, and
operations.

---

## Development

Start with **[`DEVELOPMENT.md`](DEVELOPMENT.md)**: prerequisites, build
commands, what every test proves, how to run a server locally, and the
release procedure.

```sh
cargo build && cargo test          # shared + server, any OS
```

```powershell
cd installer                        # Windows, MSVC
pwsh -File tests\Run-AllTests.ps1   # lint, tests, size gate, GUI suite
```

### Layout

| Path | What it is |
|---|---|
| `shared/` | Protocol types shared by both sides. serde only — no I/O, no HTTP. |
| `installer/` | The Windows GUI. Win32 + WinHTTP, no GUI framework, hard 600 kB gate. |
| `installer/src/` | `wizard/` (the wizard pages and its two routes), `tree/` / `choices/` (their models), `http/` (WinHTTP), `api/`, `cache/`, `sha256.rs`, `runlog/`. |
| `installer/tests/` | Every installer check: Rust tests, the size gate, and the PowerShell suite that drives the real UI. |
| `installer/res/installer.rc` | Dialog templates, all user-visible strings per language, icon, version info. |
| `server/` | axum + libSQL package and auth server. |
| `deploy/` | Containerfile, Podman Quadlet, Caddy block and the deployment guide. |
| `legacy/` | The original C# sources, kept as a behavioural reference for the port. |

### Rules and plans

Read the `CLAUDE.md` for the directory you are about to change. They hold
the decisions that are not visible in the code.

| Document | Covers |
|---|---|
| [`CLAUDE.md`](CLAUDE.md) | Architecture rules: the size budget, the install model, how the server's folder tree becomes the installer's UI tree, the auth model, the protocol. |
| [`installer/CLAUDE.md`](installer/CLAUDE.md) | Installer rules and module map. |
| [`installer/DESIGN.md`](installer/DESIGN.md) | The seven-scene wizard, screen by screen, and the Win32 behaviours worked around to build it. |
| [`server/CLAUDE.md`](server/CLAUDE.md) | Server rules and the security invariants around package lookup. |
| [`installer/tests/README.md`](installer/tests/README.md) | How tests here are written, and the traps in driving Win32 from a test. |
| [`TASKS.md`](TASKS.md) | Ordered milestones, M0-M7, plus per-crate lists in `installer/TASKS.md` and `server/TASKS.md`. |
| [`CHANGELOG.md`](CHANGELOG.md) | What changed and why. |

### Contributing

**To the CoDriver content** — this repository is only the installer. For
bugs in the pacenotes themselves, or to contribute a co-driver, go to the
[Richard Burns Rally CoPilot source repository](https://github.com/HighPriest/RBR_copilot_base).

**To the installer or the server** — pull requests welcome. Run
`installer/tests/Run-AllTests.ps1` (or `cargo test` at the root for
server-side work) before opening one, and keep the task lists and
`CHANGELOG.md` up to date with what you did.

### License

[MIT](LICENSE) — © 2026 HighPriest. Also declared as `license = "MIT"` in
the workspace `Cargo.toml`, which is what the crate metadata carries.
