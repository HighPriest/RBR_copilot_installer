//! The per-game configuration step: what a game needs done to its own files
//! once the packages are on disk. No shared abstraction on purpose, since the
//! games share no file layout; the one contract is that a step reports every
//! file it wrote, so the engine ledgers and backs those up like extracted ones.
//!
//! RBR's step edits `language` and `sounds` in the co-driver's own
//! `PaceNote.ini` and keeps every other line, because the packages now ship
//! the files the legacy `CreateRBRConfiguration` generated.

mod configure;
mod ini;
mod rbr;

pub use configure::{configure, ConfigureError, Configured};
