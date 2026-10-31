//! The installer's Win32-free logic, as a library so `cargo test` runs it on any host.
//! The Windows-only binary (`main.rs`) owns `http`, `ui` and `i18n`.
// A panic in a GUI built with `panic = "abort"` is a vanished window, so final
// code reports failures instead; tests may still assert by panicking.
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

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
pub mod utc_time;
pub mod zip;

mod relative_path;

#[cfg(test)]
mod scratch_dir;
