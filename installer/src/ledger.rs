//! Install ledger: what this installer put into a given game directory.
//! One ledger per game dir, at Plugins/Pacenote/installer/installed.json
//! (Windows separators in the real path; see install.rs). Task I7.
//!
//! Shape (root CLAUDE.md "Ledger"):
//!   { "packages": { "<directory>/<name>": {
//!       "sha256": ..., "version": ..., "files": ["relative\path", ...] } } }
//!
//! Written atomically (temp + rename) AFTER each successful package
//! install/uninstall — never once at the end. A failed Apply must leave the
//! ledger truthful.
//!
//! Cross-platform testable: base path injected, no Win32. JSON isolated here.

#![allow(dead_code)]

// TODO(I7):
//  - struct Ledger { packages: BTreeMap<String, LedgerEntry> }
//  - LedgerEntry { sha256, version, files: Vec<String> }
//  - load(game_dir) / save_atomic(game_dir)
//  - owner_of(relative_path) -> Option<&str>  (drives first-touch backup)
