//! Everything but the CLI entry point, so `server/tests/` drives the real
//! router and admin commands.

pub mod access_log;
pub mod admin;
pub mod auth;
pub mod db;
pub mod error;
pub mod reports;
pub mod routes;
pub mod store;
