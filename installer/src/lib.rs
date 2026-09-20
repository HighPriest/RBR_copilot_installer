//! Pure logic of the Pacenotes installer, as a library target.
//!
//! WHY a lib next to the bin: installer/CLAUDE.md requires the non-UI modules
//! to be `cargo test`-able without Win32. The binary (`main.rs`) is
//! `#![cfg(windows)]` and owns `http`/`ui`/`i18n`; everything here must stay
//! free of windows-sys so `cargo test --lib` runs anywhere.
//!
//! Engine planning (diff + exclusive groups + requires) is THE most
//! test-worthy logic in the installer — keep it here, keep it injectable.

pub mod cache;
pub mod engine;
pub mod gamedir;
pub mod install;
pub mod ledger;
pub mod sha256;
pub mod zip;
