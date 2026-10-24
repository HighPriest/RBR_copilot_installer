//! A zip reader over miniz_oxide, for game-root overlays: what is in a package,
//! and putting it on disk where it says. No entry is ever interpreted, and there
//! is no writer, since backups are plain file copies.
//!
//! Paths are `/`-separated on every host, because they are also the ledger's keys.
//! An archive and each entry are held in memory whole; real packages stay under 50 MB.

mod central_directory;
mod crc32;
mod entry_data;
mod entry_name;
mod extraction;
mod zip_error;

pub use entry_name::safe_relative;
pub use extraction::{entries, extract};
pub use zip_error::ZipError;

#[cfg(test)]
mod tests;
