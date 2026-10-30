//! The per-user package cache: sha-keyed payloads plus a `cache.json` index,
//! so the Download tree works offline. Its root is injected, never resolved here.
//!
//! TODO(I5): `Cache` with load/save (atomic), `payload_path(sha)`, per-package
//! state, and the orphan sweep of I9.
