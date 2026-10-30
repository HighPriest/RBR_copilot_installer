//! The Download scene's model: manifest + cache → categories, levels and leaves
//! with checkboxes. No Win32 and no display strings, so it is testable anywhere.

mod arena;
mod build;
mod cached_package;
mod node;
mod selection;

pub use arena::Tree;
pub use cached_package::CachedPackage;
pub use node::{Check, LeafState, Node};

#[cfg(test)]
mod tests;
