//! A std-only SHA-256, because the `sha2` crate's generic stack costs tens of kB.
//!
//! TODO(I5): implement.

pub struct Sha256 {}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Self {}
    }

    pub fn update(&mut self, _data: &[u8]) {}

    /// Panics until implemented, since a plausible wrong digest would defeat download verification.
    pub fn finalize(self) -> [u8; 32] {
        unimplemented!("sha256 not implemented (task I5)")
    }
}

pub fn hex(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
