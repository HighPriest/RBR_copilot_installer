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
5. serde_json is allowed but on notice: if the final binary is over budget,
   the fallback plan is replacing it with a ~100-line minimal JSON parser for
   Manifest only. Keep JSON usage isolated in `src/api.rs` so this swap stays
   cheap.
6. Prefer `windows-sys` (thin declarations) over `windows` (heavy).

## Module map

- `main.rs` — WinMain, common-controls init, dialog loop, message routing.
- `ui.rs` — dlgproc(s), control IDs (must match `res/installer.rc`), progress
  bar + status text updates, folder picker for the RBR install dir.
- `http.rs` — WinHTTP wrapper: `get(url, bearer: Option<&str>) -> Vec<u8>`
  with a progress callback. Synchronous; always called from the worker thread.
- `api.rs` — talks `pacenotes_shared` types: fetch manifest, download package
  (streams to temp file, reports progress, verifies sha256).
- `sha256.rs` — small std-only sha256 (~100 lines) for download verification.
  (No `sha2` crate: pulls generic-array/digest stack, costs tens of kB.)
- `zip.rs` — zip reader (central directory + miniz_oxide inflate) and the zip
  *writer* needed for backups (store-only/deflate, minimal).
- `install.rs` — ported C# logic. See porting map below.
- `i18n.rs` — string table lookup (localization lives in the .rc STRINGTABLE,
  selected at startup; mirrors the C# LocalizationChangeForm).

## Porting map (C# → Rust), from legacy/DownloadManager.cs

| C# | Rust | Notes |
|---|---|---|
| `InitializeSupabase[/(privateKey)]` | `api::connect` / token field | Supabase gone; access key = bearer string kept in memory; unlocks the directories granted to it |
| `ListAllFiles` + `recursiveSupabaseFileListing` | `api::fetch_manifest` | Server returns a flat manifest — no recursion needed |
| `DownloadFile` | `api::download_package` | WinHTTP, progress via PostMessageW, sha256 verify |
| `saveFile` skip-if-newer check | compare `PackageInfo.updated_at` vs local file mtime | same semantics |
| `ExtractZipFile` | `zip::extract(bytes, dir)` | preserve dir-entry handling behavior |
| `CreateRBRConfiguration` | `install::write_rbr_config` | Reproduce .ini text EXACTLY incl. `\\` paths and style→ini filename mapping |
| `backupRBRConfiguration` | `install::backup` | Keep quirks: timestamped fallback name; only write zip if some file was missing (`backup` flag). NOTE: the C# has a bug (`streamWriter.Write(entryStream)` writes the stream's ToString, not file contents) — in Rust, write the actual file bytes; flag this deviation in a code comment. |
| `unpackRBRConfiguration` | `install::unpack` | zips → extract; files → move; same `backup\FilipekMod` → `backup\unpacked` layout |
| `installRBRConfiguration` | `install::install` | copy unpacked → game root |
| `uninstallRBRConfiguration` | `install::uninstall` | delete files listed by unpacked backup |
| `restoreRBRBackup` | `install::restore_backup` | oldest zip in `backup\` by creation time — keep this rule |
| `installRBRPackage` | `install::install_package_zip` | |

UI flow to mimic (from InstallationForm): language picker dialog first (OK/
Cancel; Cancel exits — same as Program.cs), then main dialog: package list
(checkboxes, grouped by access directory), target-dir picker, access-key
entry, progress bar +
status line, install/uninstall buttons.

## Error handling

- Scaffolding phase: unwrap() tolerated.
- Final code: every user-reachable failure → `MessageBoxW` with a localized
  string, then either recover or clean exit. No silent catch-and-continue
  (the C# did this; don't copy it).

## Size audit workflow

- `check-size.ps1` — fails build over 500 kB (release gate).
- When investigating: `cargo bloat --release --target x86_64-pc-windows-msvc`
  (dev machine only; not a dependency).
- Known budget: WinHTTP+dialog skeleton ~120-180 kB; miniz ~40 kB;
  serde+serde_json ~80-120 kB; install logic ~30 kB. Headroom is real but
  finite — check after each milestone.

## Testing on non-Windows

Pure-logic modules (`zip.rs`, `sha256.rs`, `install.rs` path/ini generation)
must be testable with `cargo test` cross-platform: keep Win32 calls out of
them, inject paths/writers. Only `main/ui/http` may touch windows-sys.
