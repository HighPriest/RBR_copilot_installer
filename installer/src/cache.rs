//! The package cache inside the game folder: payloads named `<sha256>.zip`,
//! which gives integrity, dedupe and an old version still reachable after the
//! index has moved on, plus a `cache.json` index keyed by `game/directory/name`.
//! The root is injected, so it is testable anywhere.

mod index;
mod location;
mod payloads;

pub use index::{Cache, IndexState};
pub use location::dir_for;

const INDEX_FILE: &str = "cache.json";

#[cfg(test)]
mod tests;
