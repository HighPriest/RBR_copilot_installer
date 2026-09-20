//! Per-user package cache. Downloads land here sha-keyed, so integrity and
//! dedupe come free and the Install view works fully OFFLINE. Task I5.
//!
//! Layout (root CLAUDE.md "Cache"):
//!   %LOCALAPPDATA%/PacenotesInstaller/cache/<sha256>.zip   payloads
//!   %LOCALAPPDATA%/PacenotesInstaller/cache/cache.json     index
//!
//! The index maps (directory, name) -> a snapshot of the manifest's
//! PackageInfo (sha256, version, size, updated_at, category, display_name,
//! exclusive_group, requires).
//!
//! Cross-platform testable: the cache root is INJECTED, never resolved via
//! Win32 in this module. Only the caller asks Windows where LOCALAPPDATA is.
//! JSON stays isolated here (with api.rs + ledger.rs) so serde_json can be
//! swapped for a hand-rolled parser if the size budget demands it.

#![allow(dead_code)]

// TODO(I5):
//  - struct Cache { root: PathBuf, index: Index }
//  - Index { entries: BTreeMap<String /*"dir/name"*/, CachedPackage> }
//  - load/save (atomic temp + rename), payload_path(sha) -> root/<sha>.zip
//  - state(pkg) -> NotCached | Cached | UpdateAvailable (manifest sha != cached)
//  - orphan sweep: <sha>.zip files no index entry references (I9)
