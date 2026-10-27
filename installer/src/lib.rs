//! The installer's Win32-free logic, as a library so `cargo test` runs it on any host.
//! The Windows-only binary (`main.rs`) owns `http`, `ui` and `i18n`.

pub mod cache;
pub mod choices;
pub mod engine;
pub mod gamedir;
pub mod install;
pub mod ledger;
pub mod restore;
pub mod runlog;
pub mod schema;
pub mod sha256;
pub mod size;
pub mod tree;
pub mod zip;

mod relative_path;

#[cfg(test)]
mod scratch_dir;
