//! Protocol types shared between installer and server.
//!
//! RULES (see root CLAUDE.md):
//! - serde structs/enums only; no I/O, no async, no HTTP.
//! - Timestamps are unix seconds (i64), never chrono types.
//! - Any change here is a protocol change: update both sides in the same PR.
//!
//! ACCESS MODEL: packages live in named directories. The directory named
//! `PUBLIC_DIR` is visible to everyone; every other directory is visible only
//! to credentials granted that directory. One credential can hold grants for
//! any number of directories. There are no tiers.

use serde::{Deserialize, Serialize};

/// Directory that is always accessible, token or not.
pub const PUBLIC_DIR: &str = "public";

/// One installable package as listed by `GET /packages`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PackageInfo {
    /// Access directory this package belongs to (also its on-disk subdir).
    /// Lowercase [a-z0-9_-]+; validated server-side.
    pub directory: String,
    /// Name unique WITHIN its directory; also the download path segment and
    /// the on-disk filename stem (`<packages>/<directory>/<name>.zip`).
    pub name: String,
    pub version: String,
    /// Size of the package zip in bytes (for progress bars / free-space checks).
    pub size: u64,
    /// Lowercase hex sha256 of the zip. Installer MUST verify after download.
    pub sha256: String,
    /// Unix seconds of last modification (replaces Supabase `UpdatedAt`,
    /// used by the installer's "skip if already up to date" check).
    pub updated_at: i64,
    /// Optional human description shown in the UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl PackageInfo {
    pub fn is_public(&self) -> bool {
        self.directory == PUBLIC_DIR
    }
}

/// Response body of `GET /packages`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Manifest {
    pub packages: Vec<PackageInfo>,
}

impl Manifest {
    /// Keep only packages visible to a caller holding `grants`
    /// (directory names). `public` is always kept. Used by the server;
    /// exercised by tests here.
    pub fn visible_to(mut self, grants: &[String]) -> Self {
        self.packages
            .retain(|p| p.is_public() || grants.iter().any(|g| g == &p.directory));
        self
    }

    /// Anonymous view.
    pub fn public_only(self) -> Self {
        self.visible_to(&[])
    }
}

/// Uniform error body for non-2xx responses.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApiError {
    pub code: String,    // e.g. "unauthorized", "forbidden", "not_found"
    pub message: String, // human-readable, safe to show in a MessageBox
}

/// Header name carrying the bearer token ("Authorization: Bearer <token>").
pub const AUTH_HEADER: &str = "Authorization";
pub const BEARER_PREFIX: &str = "Bearer ";

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(dir: &str, name: &str) -> PackageInfo {
        PackageInfo {
            directory: dir.into(),
            name: name.into(),
            version: "1.0.0".into(),
            size: 123,
            sha256: "ab".repeat(32),
            updated_at: 1_700_000_000,
            description: None,
        }
    }

    fn sample() -> Manifest {
        Manifest {
            packages: vec![
                pkg(PUBLIC_DIR, "FilipekMod_numeric_standard"),
                pkg("premium-voices", "FilipekMod_premium_pack"),
                pkg("beta", "FilipekMod_beta_pack"),
            ],
        }
    }

    #[test]
    fn json_round_trip() {
        let m = sample();
        let s = serde_json::to_string(&m).unwrap();
        let back: Manifest = serde_json::from_str(&s).unwrap();
        assert_eq!(back.packages.len(), 3);
        assert_eq!(back.packages[1].directory, "premium-voices");
    }

    #[test]
    fn public_only_keeps_public_dir() {
        let m = sample().public_only();
        assert_eq!(m.packages.len(), 1);
        assert!(m.packages[0].is_public());
    }

    #[test]
    fn visible_to_unions_grants_with_public() {
        let m = sample().visible_to(&["beta".to_string()]);
        let dirs: Vec<_> = m.packages.iter().map(|p| p.directory.as_str()).collect();
        assert_eq!(dirs, vec![PUBLIC_DIR, "beta"]);
    }

    #[test]
    fn multiple_grants() {
        let m = sample().visible_to(&["beta".to_string(), "premium-voices".to_string()]);
        assert_eq!(m.packages.len(), 3);
    }
}
