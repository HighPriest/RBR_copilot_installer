//! RBR-specific install knowledge. The ONE built-in special step is
//! `write_rbr_config` (port of legacy DownloadManager.cs CreateRBRConfiguration);
//! everything else about installing is the generic overlay+ledger mechanism in
//! `engine.rs`. Never interpret or execute package content (root CLAUDE.md).
//!
//! Pure file-system logic: NO Win32 calls, so it's cross-platform testable.
//! Progress reporting is injected (`report: &mut dyn FnMut(u8, &str)`),
//! replacing the C# BackgroundWorker. Task I7.

#![allow(dead_code)]

pub type Report<'a> = &'a mut dyn FnMut(u8 /*percent*/, &str /*status*/);

/// Where the Pacenote plugin keeps its config, relative to the game dir.
/// Windows separators are intentional (RBR expects them) — do not normalize.
pub const PACENOTE_CONFIG_DIR: &str = "Plugins\\Pacenote\\config";

/// Port of CreateRBRConfiguration. Writes PaceNote.ini + the per-pacenote
/// Rbr*.ini config files. The generated text must match the C# output
/// byte-for-byte (modulo the date line). Returns the summary string shown
/// in the UI, same as C#.
///
/// style: "Numeric Standard" | "Numeric 1to7" | "Descriptive"
/// If `PACENOTE_CONFIG_DIR` is missing → Err with the localized
/// "install Pacenote plugin first" message (C# showed a MessageBox inline;
/// here the caller shows it).
///
/// Triggered after Apply when a `configs`-category package is installed.
pub fn write_rbr_config(
    _game_dir: &std::path::Path,
    _copilot: &str,
    _style: &str,
    _language: &str,
) -> Result<String, String> {
    // TODO(I7)
    Err("not implemented".into())
}
