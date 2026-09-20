//! Pacenotes server library: everything except the CLI entry point lives
//! here, so integration tests (`server/tests/`) can spin the real router
//! and drive the real admin commands instead of a copy of them.
//!
//! See server/CLAUDE.md for the access model and security invariants.

pub mod admin;
pub mod auth;
pub mod db;
pub mod error;
pub mod routes;
pub mod store;
