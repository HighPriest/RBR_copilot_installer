# installer/CLAUDE.md — Windows GUI installer

Windows-only (x86_64-pc-windows-msvc). HARD RELEASE GATE: final .exe < 500 kB.
Run `check-size.ps1` after every change that touches dependencies or adds
noticeable code. If size regresses, fix it before continuing.

## Why the binary stays small — do not break these

1. **HTTPS = WinHTTP** (`Win32_Networking_WinHttp`). The OS provides TLS
   (Schannel). Never add reqwest/hyper/rustls/native-tls/openssl.
2. **UI = raw Win32 dialogs** from `res/installer.rc` templates, compiled by
   `embed-resource` in build.rs, driven by `DialogBoxParamW`/`CreateDialogParamW`
   and a dlgproc. Never add a GUI framework crate.
3. **Zip = miniz_oxide** + a hand-rolled zip central-directory parser in
   `src/zip.rs` (~150 lines). Do not add the `zip` crate.
4. **No tokio.** Concurrency = one `std::thread` worker + `PostMessageW` to
   the UI thread (custom `WM_APP+n` messages carry progress %, status string
   index, completion). This replaces the C# BackgroundWorker pattern.
5. serde_json is allowed but on notice (also used for cache.json + ledger):
   if the final binary is over budget, the fallback is a minimal hand-rolled
   JSON parser/writer. Keep all JSON usage isolated in `api.rs` + `cache.rs`
   + `ledger.rs` so the swap stays cheap.
6. Prefer `windows-sys` (thin declarations) over `windows` (heavy).
7. No TOML in the installer: sidecar metadata is server-side only; the
   installer sees it pre-merged in the manifest JSON.

## Install model (see root CLAUDE.md for rationale)

- Packages are game-root overlay zips. ONE install mechanism: extract into
  game dir, record files in the ledger. NEVER interpret or execute anything
  found inside a package.
- **Cache** (`cache.rs`): `%LOCALAPPDATA%\PacenotesInstaller\cache\`
  - `<sha256>.zip` — the payloads, keyed by manifest sha (integrity+dedupe)
  - `cache.json` — index: (directory, name) → {sha256, version, size,
    updated_at, category, display_name, exclusive_group, requires} — a
    snapshot of PackageInfo, so the Install view works fully OFFLINE.
  - Download = fetch → sha256 verify → move into cache. Re-download only when
    manifest sha differs from cached sha.
- **Ledger** (`ledger.rs`): per game dir,
  `Plugins\Pacenote\installer\installed.json`:
  `{ "packages": { "<directory>/<name>": { "sha256": ..., "version": ...,
     "files": ["relative\\path", ...] } } }`
  Written atomically (temp + rename) AFTER each successful package
  install/uninstall, not once at the end.
- **Backup**: before writing a file that exists on disk but is NOT owned by
  any ledger entry, copy it to `backup\<relative path>` (first time only —
  never overwrite an existing backup). Restore = copy backup tree back.
  This replaces the legacy backup/FilipekMod/unpacked staging AND the
  oldest-zip restore rule.
- **Apply** (`engine.rs`): input = user's selection (set of cached package
  refs) + ledger. Plan = uninstall (ledgered but unselected, plus any
  ledgered member of an exclusive_group superseded by a new selection) then
  install (selected but not ledgered, or sha changed = update). Enforce
  `requires` (selecting a package auto-selects/blocks on its deps being
  cached). Show the plan in the status area before running.

## Module map

- `main.rs` — WinMain, common-controls init, dialog loop, message routing.
- `ui.rs` — dlgprocs. TWO views in the main dialog (tab or toggle):
  Library (server manifest grouped by `category`, per-row Download/state) and
  Install (game-dir picker + cached packages as checkboxes, radio-style
  within an `exclusive_group`, Apply/Restore buttons). Progress bar + status
  shared.
- `http.rs` — WinHTTP wrapper: `get(url, bearer: Option<&str>) -> Vec<u8>`
  with a progress callback. Synchronous; always called from the worker thread.
- `api.rs` — fetch manifest, download into cache (JSON isolated here).
- `cache.rs` — cache dir layout, cache.json read/write, sha verification.
- `ledger.rs` — installed.json read/write (atomic), file ownership queries.
- `engine.rs` — plan + execute Apply; backup; restore; uses zip.rs.
- `sha256.rs` — small std-only sha256 (~100 lines). (No `sha2` crate.)
- `zip.rs` — zip reader (central directory + miniz_oxide inflate). The
  legacy zip WRITER for backups is no longer needed (backups are plain
  file copies) — do not write one.
- `install.rs` — `write_rbr_config` port (the one built-in special step)
  + RBR-specific path knowledge.
- `i18n.rs` — string table lookup (localization via .rc STRINGTABLE).

## Porting map (C# legacy/DownloadManager.cs → new model)

| C# | Fate |
|---|---|
| `InitializeSupabase[/(privateKey)]` | replaced: `api.rs` + bearer access key |
| `ListAllFiles` + recursion | replaced: flat manifest fetch |
| `DownloadFile`/`saveFile` | replaced: `api.rs` download → cache (sha compare supersedes the updated_at mtime check) |
| `ExtractZipFile` | ported: `zip.rs` extract, same dir-entry handling |
| `CreateRBRConfiguration` | PORTED as `install::write_rbr_config` — ini text byte-for-byte (modulo date); may shrink where config packages already ship inis |
| `backupRBRConfiguration` | superseded by engine backup (per-file, first-touch). The C# stream bug is moot |
| `unpack/install/uninstall RBRConfiguration` | superseded by cache + ledger + engine |
| `restoreRBRBackup` (oldest-zip rule) | superseded by backup-tree restore |

Behavior users depend on and MUST survive: overlay layout lands files in the
right places; a pristine-restore path exists; config style choice produces
the same inis; missing Pacenote plugin dir → localized "install the plugin
first" error.

## Error handling

- Scaffolding phase: unwrap() tolerated.
- Final code: every user-reachable failure → `MessageBoxW` with a localized
  string, then either recover or clean exit. No silent catch-and-continue
  (the C# did this; don't copy it).
- A failed Apply must leave the ledger truthful: ledger updates are per-file
  batch per package, written after that package's action succeeds.

## Size audit workflow

- `check-size.ps1` — fails build over 500 kB (release gate).
- When investigating: `cargo bloat --release --target x86_64-pc-windows-msvc`.
- Budget sketch: WinHTTP+dialog skeleton ~120-180 kB; miniz ~40 kB;
  serde+serde_json ~80-120 kB; cache/ledger/engine ~40 kB. Headroom is real
  but finite — check after each milestone.

## Testing on non-Windows

Pure-logic modules (`zip.rs`, `sha256.rs`, `cache.rs` index logic,
`ledger.rs`, `engine.rs` planning, `install.rs` ini generation) must be
testable with `cargo test` cross-platform: keep Win32 calls out of them,
inject base paths/writers. Only `main/ui/http` may touch windows-sys.
Engine planning (diff + exclusive groups + requires) is THE most
test-worthy logic in the installer — exhaustive table tests.
