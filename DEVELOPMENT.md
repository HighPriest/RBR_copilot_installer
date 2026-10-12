# Development environment, build & test

Split: **Windows 10 = installer** (MSVC-only target), **Ubuntu server =
server + shared** (and CI-style checks). VSCode connects to both.

## 1. Windows 10 (installer)

1. Install Rust: https://rustup.rs → `rustup-init.exe`, defaults
   (`stable-x86_64-pc-windows-msvc`).
2. It will prompt for MSVC prerequisites — accept, or install manually:
   **Visual Studio Build Tools** with "Desktop development with C++"
   (needed for link.exe and the resource compiler used by embed-resource).
3. VSCode extensions: `rust-lang.rust-analyzer`, `vadimcn.vscode-lldb`
   (debugging), `ms-vscode-remote.remote-ssh` (for the server side).
4. Build + size gate (PowerShell):

       cd installer
       .\tests\Test-BinarySize.ps1        # release build, fails > 500 kB

5. Tests (pure-logic modules: zip, sha256, install):

       cd installer
       cargo test

## 2. Linux server (server + shared)

    sudo apt update && sudo apt install -y build-essential pkg-config git
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # defaults
    source "$HOME/.cargo/env"

Build & test the workspace (shared + server; installer is excluded on
purpose — Windows-only):

    cargo build
    cargo test

Run the server locally:

    mkdir -p packages/public data
    cargo run -p pacenotes-server -- serve --listen 127.0.0.1:8080
    curl http://127.0.0.1:8080/health     # → ok

## 3. VSCode ↔ remote workflow

- Open the repo on the Ubuntu box via **Remote-SSH** for server/shared work;
  rust-analyzer runs remotely, nothing to install locally beyond the extension.
- Open the same repo locally on Windows for installer work. Keep one git
  remote (e.g. Gitea/GitHub) and sync via push/pull — don't edit the same
  tree over SMB/SSHFS; rust-analyzer + target/ over network mounts is painful.
- Two clones = two `target/` dirs; that's expected.

## 4. First-run order (matches TASKS.md M0/M1)

1. Ubuntu: `cargo test -p pacenotes-shared` → green.
2. Ubuntu: `cargo build -p pacenotes-server` → fix/pin versions, commit
   `Cargo.lock`.
3. Windows: `rustup target list --installed` should show
   `x86_64-pc-windows-msvc`; then `installer\tests\Test-BinarySize.ps1`

## 5. Testing against the server from Windows

Point the installer's `Api::base_url` (or later a config flag) at
`http://<server-ip>:8080` for plain-HTTP dev. WinHTTP requires HTTPS only if
the URL says https — keep dev on http, staging/prod behind Caddy TLS.
