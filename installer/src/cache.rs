//! The per-user package cache: payloads named `<sha256>.zip`, which gives
//! integrity and dedupe, plus a `cache.json` index keyed by
//! `game/directory/name`. The root is injected, so it is testable anywhere.

mod index;
mod location;
mod payloads;

pub use index::Cache;
pub use location::default_root;

const INDEX_FILE: &str = "cache.json";

#[cfg(test)]
mod tests;
