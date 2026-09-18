//! Package store: manifest.json (source of truth for metadata) + zips on disk
//! under per-access-directory subdirs: <packages>/<directory>/<name>.zip.
//! Task S3/S4. The DB never knows about packages; grants reference directory
//! names only.

#![allow(dead_code)]

use pacenotes_shared::{Manifest, PackageInfo};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub struct ManifestStore {
    packages_dir: PathBuf,
    cache: RwLock<Cached>,
}

struct Cached {
    manifest: Manifest,
    mtime: Option<std::time::SystemTime>,
}

impl ManifestStore {
    pub fn new(packages_dir: impl Into<PathBuf>) -> Self {
        Self {
            packages_dir: packages_dir.into(),
            cache: RwLock::new(Cached {
                manifest: Manifest::default(),
                mtime: None,
            }),
        }
    }

    fn manifest_path(&self) -> PathBuf {
        self.packages_dir.join("manifest.json")
    }

    /// Lazy reload: compare manifest.json mtime; reload under write lock if
    /// changed. Called at the top of each handler. Task S3.
    pub fn current(&self) -> Manifest {
        // TODO(S3): mtime check + reload; scaffold returns cached copy.
        self.cache.read().unwrap().manifest.clone()
    }

    /// SECURITY: `directory` and `name` are lookup keys into the manifest,
    /// never path fragments from the URL. Returns the on-disk zip path only
    /// for known (directory, name) pairs — this is the structural
    /// path-traversal defense (CLAUDE.md). The manifest generator (below) is
    /// the only place directory/name strings are created, and it validates
    /// them against [a-z0-9_-]+ / no separators.
    pub fn zip_path(&self, directory: &str, name: &str) -> Option<(PackageInfo, PathBuf)> {
        let m = self.current();
        let pkg = m
            .packages
            .into_iter()
            .find(|p| p.directory == directory && p.name == name)?;
        let path = self
            .packages_dir
            .join(&pkg.directory)
            .join(format!("{}.zip", pkg.name));
        Some((pkg, path))
    }

    /// `manifest gen` implementation target (task S3): scan
    /// <dir>/<subdir>/*.zip — subdir name = access directory (validate
    /// [a-z0-9_-]+; skip and warn otherwise). Compute size/sha256/mtime,
    /// preserve version/description of existing entries, write manifest.json.
    /// Top-level zips (no subdir) are an error: every package must live in a
    /// directory; `public/` is where anonymous packages go.
    pub fn generate(_dir: &Path) -> anyhow::Result<Manifest> {
        // TODO(S3)
        anyhow::bail!("not implemented (task S3)")
    }
}
