//! Minimal zip reader/writer over miniz_oxide. Replaces System.IO.Compression.
//! No `zip` crate (size budget). Cross-platform testable — no Win32 here.
//! Task I6.
//!
//! Reader plan: locate End Of Central Directory (scan last 64 KiB for
//! PK\x05\x06), walk central directory entries (PK\x01\x02), for each local
//! header (PK\x03\x04) inflate (method 8, miniz_oxide::inflate) or copy
//! (method 0). Behavior must match C# ExtractZipFile including its
//! directory-entry handling.
//!
//! Writer plan (backups only): store or deflate entries, write local headers +
//! central directory + EOCD. CRC32 table included here (~30 lines).

#![allow(dead_code)]

use std::path::Path;

pub struct ZipError(pub String);

/// Extract an in-memory zip to `dest_dir`, creating directories as needed.
pub fn extract(_zip_bytes: &[u8], _dest_dir: &Path) -> Result<(), ZipError> {
    // TODO(I6)
    Err(ZipError("not implemented".into()))
}

pub struct ZipWriter {
    // TODO(I6): output buffer, central-directory records.
}

impl ZipWriter {
    pub fn new() -> Self {
        Self {}
    }
    /// `name` uses forward slashes per zip spec (convert from Windows paths).
    pub fn add_file(&mut self, _name: &str, _contents: &[u8]) -> Result<(), ZipError> {
        // TODO(I6)
        Err(ZipError("not implemented".into()))
    }
    pub fn finish(self) -> Vec<u8> {
        // TODO(I6)
        Vec::new()
    }
}
