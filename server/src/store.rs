//! The data root: package zips on disk, the generated manifest that lists
//! them, and the message of the day.
//!
//! ```text
//! data/rbr/packages/public/codriver/polish/male/Kamil.zip
//!      ^game        ^dir   ^------------ name -----^
//! data/motd/<lang>.txt
//! data/manifest.json
//! ```

mod derived_version;
mod file_hash;
mod generate;
mod generated;
mod manifest_file;
mod manifest_store;
mod metadata_losses;
mod motd;
mod requires_check;
mod sidecar;
mod sidecar_migration;

#[cfg(test)]
mod fixture_tree;

pub use derived_version::{derived_version, is_chosen_version, DERIVED_VERSION_PREFIX};
pub use file_hash::sha256_file;
pub use generate::generate;
pub use generated::Generated;
pub use manifest_file::{read_manifest, write_manifest};
pub use manifest_store::{zip_file, ManifestStore};
pub use metadata_losses::metadata_losses;
pub use motd::{valid_lang, MOTD_DIR, MOTD_FALLBACK_LANG};
pub use sidecar_migration::{migrate_to_sidecars, Migration};

/// The folder under each game that `manifest gen` scans; anything else in a game folder is ignored.
pub const PACKAGES_DIR: &str = "packages";

pub const MANIFEST_FILE: &str = "manifest.json";
