//! The apply engine: plan from selection + ledger + cache, then execute.
//!
//! The plan uninstalls everything superseded before installing anything,
//! because the three style packages ship the same 49 paths and the other order
//! would delete what was just written. Execution backs up, installs, then runs
//! the game's configuration step (`install.rs`), whose files are ledgered too.
//!
//! A file about to be overwritten that no ledger entry owns is copied once to
//! `.pacenotes\backup\<relative path>`, unless its bytes already match its
//! replacement, which keeps a first run over a manual install from copying
//! thousands of identical files. Restore copies that tree back.
//!
//! TODO(I7): `plan()`, `execute()`, `backup_file()`, `restore_backup()`.
