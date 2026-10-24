//! The install ledger: what this installer put into a game folder, at
//! `.pacenotes\installed.json` for every game, outside any plugin tree that
//! packages or other installers also write to. Rewritten atomically after each package.
//!
//! ```text
//! { "packages": { "<game>/<directory>/<name>": {
//!     "sha256": ..., "version": ..., "exclusive_group": ...,
//!     "files": ["relative/path", ...] } } }
//! ```
//!
//! Paths are `/`-separated as `zip::entries` returns them, and `exclusive_group`
//! is recorded as installed so uninstalling works offline.
//!
//! TODO(I7): `Ledger` with load/save, `LedgerEntry { sha256, version,
//! exclusive_group, files }`, and `owners_of(relative_path)`, which drives the
//! first-touch backup (no owner: the user's file) and uninstall (deleted only
//! when no other entry lists it, since all three styles own the same 49 paths).

/// Everything the installer owns inside a game folder, under one directory.
pub const STATE_DIR: &str = ".pacenotes";
