//! A minimal zip reader over miniz_oxide, matching the legacy `ExtractZipFile`.
//! There is no writer: backups are plain file copies.
//!
//! TODO(I6): implement both functions.

pub struct ZipError(pub String);

/// Entry paths in zip order, without extracting.
pub fn entries(_zip_bytes: &[u8]) -> Result<Vec<String>, ZipError> {
    Err(ZipError("not implemented".into()))
}

/// Must refuse any entry that would land outside `dest_dir`.
pub fn extract(_zip_bytes: &[u8], _dest_dir: &std::path::Path) -> Result<(), ZipError> {
    Err(ZipError("not implemented".into()))
}
