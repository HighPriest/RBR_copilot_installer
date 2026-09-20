//! Minimal zip READER over miniz_oxide. Replaces System.IO.Compression.
//! No `zip` crate (size budget). Cross-platform testable — no Win32 here.
//! Task I6.
//!
//! There is deliberately NO zip writer: backups are plain per-file copies
//! made by `engine.rs` (root CLAUDE.md "Backup"). Do not add one.
//!
//! Reader plan: locate End Of Central Directory (scan last 64 KiB for
//! PK\x05\x06), walk central directory entries (PK\x01\x02), for each local
//! header (PK\x03\x04) inflate (method 8, miniz_oxide::inflate) or copy
//! (method 0). Behavior must match C# ExtractZipFile including its
//! directory-entry handling.

#![allow(dead_code)]

pub struct ZipError(pub String);

/// List entry paths (relative, zip order) WITHOUT extracting.
/// `engine.rs` planning needs this to know which files a package will own.
pub fn entries(_zip_bytes: &[u8]) -> Result<Vec<String>, ZipError> {
    // TODO(I6)
    Err(ZipError("not implemented".into()))
}

/// Extract an in-memory zip to `dest_dir`, creating directories as needed.
/// MUST reject entries that escape `dest_dir` (zip-slip guard).
pub fn extract(_zip_bytes: &[u8], _dest_dir: &std::path::Path) -> Result<(), ZipError> {
    // TODO(I6)
    Err(ZipError("not implemented".into()))
}

#[cfg(test)]
mod tests {
    // TODO(I6): fixture zip in tests/data — extract + entries() listing,
    // plus a zip-slip fixture that must be rejected.
}
