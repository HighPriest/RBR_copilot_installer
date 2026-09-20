//! Minimal std-only SHA-256 (~100 lines when implemented). We deliberately do
//! NOT use the `sha2` crate: its digest/generic-array stack costs tens of kB
//! against the 500 kB budget. Task I5. Cross-platform testable (no Win32).

#![allow(dead_code)]

pub struct Sha256 {
    // TODO(I5): state [u32; 8], buffer, length counter.
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

#[cfg(test)]
mod tests {
    // TODO(I5): implement, then enable these known vectors:
    //  sha256("")      = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    //  sha256("abc")   = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
    //  sha256(1M 'a')  = cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0
}
