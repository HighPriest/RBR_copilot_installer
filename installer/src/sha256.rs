//! Minimal std-only SHA-256 (~100 lines when implemented). We deliberately do
//! NOT use the `sha2` crate: its digest/generic-array stack costs tens of kB
//! against the 500 kB budget. Task I5. Cross-platform testable (no Win32).

#![allow(dead_code)]

pub struct Sha256 {
    // TODO(I5): state [u32; 8], buffer, length counter.
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Self {}
    }
    pub fn update(&mut self, _data: &[u8]) {
        // TODO(I5)
    }
    /// NOTE: until I5 lands this panics rather than returning a plausible
    /// wrong digest — a silently-zero hash would defeat download verification.
    pub fn finalize(self) -> [u8; 32] {
        unimplemented!("sha256 not implemented (task I5)")
    }
}

pub fn hex(digest: &[u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
