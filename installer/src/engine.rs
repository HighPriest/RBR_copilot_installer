//! Apply engine: turn (user selection + ledger + cache index) into an ordered
//! plan, then execute it. THE most test-worthy logic in the installer —
//! exhaustive table tests required. Task I7.
//!
//! plan() decides two sets and orders all uninstalls before all installs:
//!
//! - uninstall: ledgered but unselected, plus any ledgered member of an
//!   exclusive_group superseded by a new selection
//! - install: selected but not ledgered, or cached sha != ledgered sha
//! - `requires` is enforced, blocking with a message if a dep is not cached
//!
//! execute() walks that plan:
//!
//! - uninstall: delete ledgered files, prune emptied dirs, drop ledger entry
//! - install: per zip entry, take a first-touch backup if an existing file is
//!   owned by no ledger entry, extract, then write the package's ledger entry
//!
//! Backup lives here too: `backup/<relative path>`, never overwriting an
//! existing backup file; restore copies the tree back over the game dir.
//!
//! Cross-platform testable: paths injected, no Win32 calls in this module.

#![allow(dead_code)]

// TODO(I7):
//  - enum Action { Uninstall(PackageRef), Install(PackageRef) }
//  - plan(selection, &Ledger, &Cache) -> Result<Vec<Action>, PlanError>
//  - execute(&[Action], game_dir, &Cache, &mut Ledger, Report) -> Result<()>
//  - backup_file(game_dir, rel) / restore_backup(game_dir, Report)
