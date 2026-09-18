//! Port of legacy DownloadManager.cs "RBRConfiguration" + install/uninstall
//! regions. Behavior-for-behavior; see the porting map in installer/CLAUDE.md.
//! Pure file-system logic: NO Win32 calls, so it's cross-platform testable.
//! Progress reporting is injected (`report: &mut dyn FnMut(u8, &str)`),
//! replacing the C# BackgroundWorker. Task I7.

#![allow(dead_code)]

use std::path::Path;

pub type Report<'a> = &'a mut dyn FnMut(u8 /*percent*/, &str /*status*/);

/// Layout constants (match C# literals exactly).
pub const BACKUP_DIR: &str = "backup";
pub const DOWNLOAD_DIR: &str = "backup\\FilipekMod";
pub const UNPACK_DIR: &str = "backup\\unpacked";

/// Port of CreateRBRConfiguration. Writes PaceNote.ini + the per-pacenote
/// Rbr*.ini config files. The generated text must match the C# output
/// byte-for-byte (modulo the date line). Returns the summary string shown
/// in the UI, same as C#.
///
/// style: "Numeric Standard" | "Numeric 1to7" | "Descriptive"
/// If the Plugins\Pacenote\config dir is missing → Err with the localized
/// "install Pacenote plugin first" message (C# showed a MessageBox inline;
/// here the caller shows it).
pub fn write_rbr_config(
    _game_dir: &Path,
    _copilot: &str,
    _style: &str,
    _language: &str,
) -> Result<String, String> {
    // TODO(I7)
    Err("not implemented".into())
}

/// Port of backupRBRConfiguration: zip up the original files that the
/// unpacked payload is about to overwrite → backup\backup_Pacenote.zip
/// (timestamped name if it already exists). Only writes the zip if at least
/// one payload file did NOT already exist (the C# `backup` flag quirk —
/// keep it, but note CAUTION comment from the original).
///
/// DELIBERATE FIX vs C#: the original wrote `streamWriter.Write(entryStream)`
/// (a ToString of the stream — corrupt backups). We write real file bytes.
pub fn backup(_game_dir: &Path, _report: Report) -> Result<(), String> {
    // TODO(I7)
    Err("not implemented".into())
}

/// Port of unpackRBRConfiguration: backup\FilipekMod → backup\unpacked.
/// Zips get extracted; loose files get moved (overwrite).
pub fn unpack(_game_dir: &Path, _report: Report) -> Result<(), String> {
    // TODO(I7)
    Err("not implemented".into())
}

/// Port of installRBRConfiguration: copy backup\unpacked\** → game root.
pub fn install(_game_dir: &Path, _report: Report) -> Result<(), String> {
    // TODO(I7)
    Err("not implemented".into())
}

/// Port of installRBRPackage: extract backup\FilipekMod\{package}.zip → root.
pub fn install_package_zip(
    _game_dir: &Path,
    _package_name: &str,
    _report: Report,
) -> Result<(), String> {
    // TODO(I7)
    Err("not implemented".into())
}

/// Port of uninstallRBRConfiguration: delete from game root every file that
/// exists under backup\unpacked (mirror of install).
pub fn uninstall(_game_dir: &Path, _report: Report) -> Result<(), String> {
    // TODO(I7)
    Err("not implemented".into())
}

/// Port of restoreRBRBackup: extract the OLDEST (by creation time) *.zip in
/// backup\ over the game root. Keep the oldest-file rule — it's intentional
/// ("first backup = pristine state").
pub fn restore_backup(_game_dir: &Path, _report: Report) -> Result<(), String> {
    // TODO(I7)
    Err("not implemented".into())
}

#[cfg(test)]
mod tests {
    // TODO(I7): tempdir-based tests for each function; write_rbr_config gets
    // exact-string assertions against the C# output (fixture in tests/data).
}
