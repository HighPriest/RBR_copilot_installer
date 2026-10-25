/// The published manifest, reloaded when `manifest.json`'s mtime changes.
pub struct ManifestStore {
    data_dir: std::path::PathBuf,
    cache: std::sync::RwLock<CachedManifest>,
}

struct CachedManifest {
    manifest: pacenotes_shared::Manifest,
    mtime: Option<std::time::SystemTime>,
    is_loaded: bool,
}

impl ManifestStore {
    pub fn new(data_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            cache: std::sync::RwLock::new(CachedManifest {
                manifest: pacenotes_shared::Manifest::default(),
                mtime: None,
                is_loaded: false,
            }),
        }
    }

    pub fn manifest_path(&self) -> std::path::PathBuf {
        self.data_dir.join(crate::store::MANIFEST_FILE)
    }

    /// A missing manifest is an empty one; an unreadable one keeps the last
    /// good copy serving, because a bad publish must not fail requests.
    pub fn current(&self) -> pacenotes_shared::Manifest {
        let disk_mtime = std::fs::metadata(self.manifest_path())
            .and_then(|metadata| metadata.modified())
            .ok();
        {
            let cached = self.cache.read().unwrap_or_else(|e| e.into_inner());
            if cached.is_loaded && cached.mtime == disk_mtime {
                return cached.manifest.clone();
            }
        }

        match crate::store::manifest_file::read_manifest(&self.manifest_path()) {
            Ok(manifest) => {
                let mut cached = self.cache.write().unwrap_or_else(|e| e.into_inner());
                cached.manifest = manifest.clone();
                cached.mtime = disk_mtime;
                cached.is_loaded = true;
                manifest
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    path = %self.manifest_path().display(),
                    "manifest load failed"
                );
                self.cache
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .manifest
                    .clone()
            }
        }
    }

    /// The zip behind a manifest entry; the URL's `game`, `directory` and
    /// `name` are only ever lookup keys, so nothing unlisted can resolve.
    pub fn zip_path(
        &self,
        game: &str,
        directory: &str,
        name: &str,
    ) -> Option<(pacenotes_shared::PackageInfo, std::path::PathBuf)> {
        let package = self.current().packages.into_iter().find(|package| {
            package.game == game && package.directory == directory && package.name == name
        })?;
        if !pacenotes_shared::valid_id(&package.game)
            || !pacenotes_shared::valid_id(&package.directory)
            || !pacenotes_shared::valid_name(&package.name)
        {
            return None;
        }
        let path = self
            .data_dir
            .join(&package.game)
            .join(crate::store::PACKAGES_DIR)
            .join(&package.directory)
            .join(format!("{}.zip", package.name));
        Some((package, path))
    }

    pub fn motd(&self, lang: &str) -> Option<String> {
        crate::store::motd::read_motd(&self.data_dir, lang)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reloads_on_mtime_change_and_resolves_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        crate::store::fixture_tree::write(
            &crate::store::fixture_tree::package_path(dir, "rbr", "public/basic.zip"),
            b"hello",
        );
        let store = super::ManifestStore::new(dir);
        assert!(store.current().packages.is_empty());

        crate::store::write_manifest(dir, &crate::store::fixture_tree::generate(dir).manifest)
            .unwrap();

        assert_eq!(store.current().packages.len(), 1);
        let (package, path) = store.zip_path("rbr", "public", "basic").unwrap();
        assert_eq!(package.name, "basic");
        assert_eq!(
            path,
            crate::store::fixture_tree::package_path(dir, "rbr", "public/basic.zip")
        );
        assert!(store.zip_path("rbr", "public", "nope").is_none());
        assert!(store.zip_path("rbr", "../data", "auth").is_none());
    }

    #[test]
    fn nested_names_resolve_back_to_their_zip() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let kamil = crate::store::fixture_tree::package_path(
            dir,
            "rbr",
            "public/codriver/polish/male/Kamil.zip",
        );
        crate::store::fixture_tree::write(&kamil, b"kamil");
        crate::store::write_manifest(dir, &crate::store::fixture_tree::generate(dir).manifest)
            .unwrap();

        let store = super::ManifestStore::new(dir);
        let (package, path) = store
            .zip_path("rbr", "public", "codriver/polish/male/Kamil")
            .unwrap();
        assert_eq!(package.leaf(), "Kamil");
        assert_eq!(path, kamil);

        for not_a_package in [
            "codriver/polish/male",
            "codriver",
            "codriver/../../manifest",
        ] {
            assert!(
                store.zip_path("rbr", "public", not_a_package).is_none(),
                "{not_a_package}"
            );
        }
    }

    #[test]
    fn a_bad_manifest_keeps_the_last_good_copy_serving() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        crate::store::fixture_tree::write(
            &crate::store::fixture_tree::package_path(dir, "rbr", "public/basic.zip"),
            b"hello",
        );
        crate::store::write_manifest(dir, &crate::store::fixture_tree::generate(dir).manifest)
            .unwrap();
        let store = super::ManifestStore::new(dir);
        assert_eq!(store.current().packages.len(), 1);

        crate::store::fixture_tree::write(&dir.join(crate::store::MANIFEST_FILE), b"{ not json");
        assert_eq!(store.current().packages.len(), 1);
    }
}
