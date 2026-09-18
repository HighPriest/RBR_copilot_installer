//! Server API layer: shared::Manifest fetch + verified package download.
//! JSON usage is isolated HERE so serde_json can be swapped for a minimal
//! parser if the size budget demands it (CLAUDE.md §5). Task I5.

#![allow(dead_code)]

use pacenotes_shared::{Manifest, PackageInfo};
use std::path::Path;
use std::sync::atomic::AtomicBool;

pub struct Api {
    pub base_url: String,          // e.g. "https://packages.example.com"
    pub token: Option<String>,     // bearer access key; set by Unlock (I8);
                                   // server filters manifest to public +
                                   // this key's granted directories
}

impl Api {
    pub fn new(base_url: &str) -> Self {
        Self { base_url: base_url.trim_end_matches('/').to_string(), token: None }
    }

    /// GET /packages → Manifest. Replaces C# ListAllFiles + recursion:
    /// the server returns a flat list, no bucket walking.
    pub fn fetch_manifest(&self, _cancel: &AtomicBool) -> Result<Manifest, String> {
        // TODO(I5): http::get(base_url + "/packages", token) → serde_json parse
        Err("not implemented".into())
    }

    /// GET /download/{directory}/{name} → temp file → sha256 verify → rename
/// into place.
    /// Skip-if-current: if `dest` exists and its mtime > pkg.updated_at, no-op
    /// (port of C# saveFile early return).
    pub fn download_package(
        &self,
        _pkg: &PackageInfo,
        _dest: &Path,
        _progress: crate::http::ProgressFn,
        _cancel: &AtomicBool,
    ) -> Result<(), String> {
        // TODO(I5)
        Err("not implemented".into())
    }
}
