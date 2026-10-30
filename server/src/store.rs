//! The data root: package zips on disk, the generated manifest that lists
//! them, and the message of the day.
//!
//! ```text
//! data/rbr/packages/public/codriver/polish/male/Kamil.zip
//!      ^game        ^dir   ^------------ name -----^
//! data/motd/<lang>.txt
//! data/manifest.json
//! ```

mod file_hash;
mod generate;
mod manifest_file;
mod manifest_store;
mod motd;
mod requires_check;
mod sidecar;

#[cfg(test)]
mod fixture_tree;

pub use file_hash::sha256_file;
pub use generate::{generate, Generated};
pub use manifest_store::ManifestStore;
pub use motd::{valid_lang, MOTD_DIR, MOTD_FALLBACK_LANG};

/// The folder under each game that `manifest gen` scans; anything else in a game folder is ignored.
pub const PACKAGES_DIR: &str = "packages";

pub const MANIFEST_FILE: &str = "manifest.json";
