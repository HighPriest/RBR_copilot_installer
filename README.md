# RBR Copilot Installer

Richard Burns Rally · an esports-ready co-pilot · the ultimate second seat filler

[![Demo video](https://i3.ytimg.com/vi/TkRz2F80gec/mqdefault.jpg)](https://youtu.be/fZ3FEQRCJog)

**Documentation:** [highpriest.github.io/RBR_copilot_installer](https://highpriest.github.io/RBR_copilot_installer/)
— everything below in more depth, the architecture, and the API reference.

## Introduction

This repository is the source of an installer for CoDriver configuration:
pacenote packages for Richard Burns Rally and, in time, other rally games.
It is public so you can see where the project stands, point out bugs, and
say what you want from it.

It is a Rust rewrite of the original C#/WinForms installer, and it comes
with its own package server instead of a third-party backend. Two things
ship from here:

- **The installer** — one small Windows `.exe`. It downloads packages,
  verifies them, keeps them in the game folder so a second run costs
  nothing, installs the ones you choose, and can put the folder back the
  way it was.
- **The server** — one binary that serves those packages as plain files
  from a folder tree, with optional access keys for private content. Its
  only database is a small SQLite file holding those keys, which it creates
  itself. There is no upload API: publishing a package is copying a zip into
  place.

## Using the installer

Download `pacenotes-installer.exe` from the
[releases page](https://github.com/HighPriest/RBR_copilot_installer/releases)
and run it. It is one file: no setup, nothing in Program Files, nothing in
the registry. It needs nothing else installed — no Visual C++
redistributable, no .NET — on Windows 10 or later.

The wizard, page by page. The button that moves you on is the green one.

1. **Language** — English or Polish; it starts in your Windows language.
2. **Message of the day** — what the server has to say, and a field for an
   access key if you have one. Leave it empty for public content. If the
   server cannot be reached you can carry on with what you already
   downloaded.
3. **Game folder** — point it at the folder that holds the game's own
   executable; the game is recognised from that. If your Windows account
   cannot write there (a game under Program Files, say), it offers to
   restart as administrator, keeping what you chose. Then pick one of three
   routes:
   - **Download/Update** — get new content, then choose and install it.
   - **Configure/Install** — change what is active, using only what is
     already on this computer: no server needed. It opens once each
     category has something downloaded.
   - **Restore/Uninstall** — put the folder back to an earlier state, or
     remove everything this installer added.
4. **Download** — a tree of everything available, laid out the way the
   packages are filed on the server, every row open. Tick what you want on
   this computer; a category with only some items ticked shows a partial
   box. What you already have is ticked and marked. Pick at least one item
   in each category the game needs (language, co-driver, style).
5. **Downloading** — progress, the package being fetched, and a live log.
   A failure a retry could fix is retried; one that will not is named, and
   the rest carry on. Cancel works here, and nothing is written to the game
   folder yet. If a category is left with nothing, you go back to step 4;
   if nothing could be downloaded at all, the run ends at the summary.
6. **Configuration** — the one pacenotes language, co-driver voice and
   style to use. Voices are listed with their full path, so two voices with
   the same name stay distinguishable. What is installed now is
   pre-selected, so changing one thing does not mean answering everything
   again.
7. **Installation** — backup and install, with the same progress screen and
   log. No server needed. It will not start while the game is running from
   this folder.
8. **Summary** — what was done, what was downloaded, and where the log is.
   The log is hidden when everything went well and shown when it did not.

**Restore/Uninstall** lists the restore points, newest first, plus
"remove everything", and previews what choosing one would put back, remove
or leave alone before anything happens.

What a run leaves on your machine:

```text
<game folder>\.pacenotes\    everything, and nothing anywhere else:
                             the downloaded packages, what was installed,
                             what was replaced, the restore points, and
                             one log per run
```

**Nothing is written outside the game folder** — not to your profile, not
to the registry, not to Program Files. Deleting the game folder removes
every trace of this installer, and copying the folder to another drive or PC
brings your downloads and restore points with it. Downloads are named by
content hash, so a second run re-uses them and works offline; two game
folders each keep their own copy, which is the price of leaving nothing
behind.

Versions before 2026-10-27 kept downloads in
`%LOCALAPPDATA%\PacenotesInstaller`. Nothing reads that folder any more, and
you can delete it.

## Deploying the package server

One Podman Quadlet runs the server image, one Caddy site block puts it on
HTTPS, and one directory holds everything that changes.

```sh
cp deploy/pacenotes.container deploy/pacenotes.config.env \
   ~/.config/containers/systemd/
systemctl --user daemon-reload && systemctl --user start pacenotes
```

**Start with [`deploy/README.md`](deploy/README.md).** It covers the data
layout (which decides what the installer's tree looks like), publishing
packages, optional metadata, the message of the day, access keys, and
operations.

## Documentation

The [documentation site](https://highpriest.github.io/RBR_copilot_installer/)
is an mdBook built from this repository on every push to `master`: every
document here, the architecture diagrams (written in [D2](https://d2lang.com)
and rendered to SVG), and the rustdoc API reference. Build it locally with
`docs/build.sh`, which needs `d2` and `mdbook` on the PATH;
[`DEVELOPMENT.md`](DEVELOPMENT.md) has the details.

| Document | Covers |
|---|---|
| [`CLAUDE.md`](CLAUDE.md) | The architecture rules: the size budget, the install model, how the server's folder tree becomes the installer's tree, the auth model, the protocol. |
| [`installer/CLAUDE.md`](installer/CLAUDE.md) | The installer's rules and module map. |
| [`installer/DESIGN.md`](installer/DESIGN.md) | The wizard's nine pages and three routes, screen by screen, and the Win32 behaviours worked around to build them. |
| [`server/CLAUDE.md`](server/CLAUDE.md) | The server's rules and the security invariants around package lookup. |
| [`DEVELOPMENT.md`](DEVELOPMENT.md) | Prerequisites, building, what every test proves, running a server locally, releasing. |
| [`installer/tests/README.md`](installer/tests/README.md) | How the installer's tests are written, and the traps in driving Win32 from a test. |
| [`TASKS.md`](TASKS.md) | Milestones M0–M7, with the detail in `installer/TASKS.md` and `server/TASKS.md`. |
| [`CHANGELOG.md`](CHANGELOG.md) | What changed, and why. |

## Development

```sh
cargo build && cargo test           # shared, server and the docs tool; any OS
```

```powershell
cd installer                        # Windows, MSVC
pwsh -File tests\Run-AllTests.ps1   # lint, tests, size gate, the GUI suite
```

| Path | What it is |
|---|---|
| `shared/` | The protocol types both sides compile against. serde only: no I/O, no HTTP. |
| `installer/` | The Windows GUI. Win32 and WinHTTP, no GUI framework, a hard 700 kB size gate. |
| `installer/src/` | The library (`tree/`, `choices/`, `cache/`, `ledger/`, `engine/`, `restore/`, `zip/`, …) and the Windows application (`wizard/`, `worker/`, `http/`, `ui/`, `i18n/`). |
| `installer/res/installer.rc` | Dialog templates, every user-visible string in each language, the icon, version info. |
| `installer/tests/` | The scenario tests, the size gate, and the PowerShell suite that drives the real UI. |
| `server/` | The axum + libSQL package and auth server. |
| `schemas/` | Each game's install schema, embedded in the installer and served by the server. |
| `deploy/` | The Containerfile, the Podman Quadlet, the Caddy block and the deployment guide. |
| `docs/` | The documentation site: the book, the D2 diagram sources, `build.sh`. |
| `tools/mdbook-repo-links/` | The book's preprocessor, which points links to source files at GitHub. |

Read the `CLAUDE.md` of the directory you are about to change: they hold
the decisions that are not visible in the code.

## Contributing

**To the CoDriver content** — this repository is only the installer. For
bugs in the pacenotes themselves, or to contribute a co-driver, go to the
[Richard Burns Rally CoPilot source repository](https://github.com/HighPriest/RBR_copilot_base).

**To the installer or the server** — pull requests are welcome. Run
`installer/tests/Run-AllTests.ps1` (or `cargo test` at the root for
server-side work) before opening one, and keep the task lists and
`CHANGELOG.md` up to date with what you did.

## License

[MIT](LICENSE) — © 2026 HighPriest. Also declared as `license = "MIT"` in
the workspace `Cargo.toml`, which is what the crate metadata carries.
