//! The install ledger: what this installer put into a game folder, at
//! `.pacenotes\installed.json` for every game, outside any plugin tree that
//! packages or other installers also write to.
//!
//! It records what was written (a sha256 per file, so a file someone has
//! since replaced is left alone), what was there before (so uninstall
//! restores rather than deletes), paths under case-folded keys (as NTFS
//! compares them), and a write-ahead state (so a run that died is recognised
//! as ours).

mod document;
mod ownership;
mod record;

pub use document::{Ledger, LedgerState};
pub use ownership::fold;
pub use record::{EntryState, LedgerEntry, LedgerFile, Prior};

/// Everything the installer owns inside a game folder, under one directory.
pub const STATE_DIR: &str = ".pacenotes";

const LEDGER_FILE: &str = "installed.json";

#[cfg(test)]
mod tests;
