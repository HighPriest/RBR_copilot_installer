//! The server's endpoints: URLs in, parsed bodies out. The transport is `http`
//! and the thread is `worker`.

mod client;
mod conflict_report;
mod url_encoding;

pub use client::Api;
