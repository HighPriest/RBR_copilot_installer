/// The index as stored; a struct, so the file can grow fields older readers ignore.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Index {
    #[serde(default)]
    packages: std::collections::BTreeMap<String, crate::tree::CachedPackage>,
}

pub struct Cache {
    pub(super) root: std::path::PathBuf,
    index: Index,
}

impl Cache {
    /// A missing or unreadable index opens as an empty cache, leaving its
    /// payloads on disk as orphans rather than failing the run.
    pub fn open(root: impl Into<std::path::PathBuf>) -> Cache {
        let root = root.into();
        let index = std::fs::read(root.join(super::INDEX_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Index>(&bytes).ok())
            .unwrap_or_default();
        Cache { root, index }
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    pub fn get(&self, key: &str) -> Option<&crate::tree::CachedPackage> {
        self.index.packages.get(key)
    }

    /// Every entry whose payload is still on disk, so nothing missing is offered as downloaded.
    pub fn entries(&self) -> Vec<crate::tree::CachedPackage> {
        self.index
            .packages
            .values()
            .filter(|entry| self.has_payload(&entry.sha256))
            .cloned()
            .collect()
    }

    pub(super) fn known_digests(&self) -> std::collections::BTreeSet<&str> {
        self.index
            .packages
            .values()
            .map(|entry| entry.sha256.as_str())
            .collect()
    }

    /// Saves at once, so an interrupted run keeps the download that just finished.
    pub fn insert(&mut self, package: crate::tree::CachedPackage) -> std::io::Result<()> {
        self.index.packages.insert(package.key(), package);
        self.save()
    }

    pub fn remove(&mut self, key: &str) -> std::io::Result<()> {
        self.index.packages.remove(key);
        self.save()
    }

    /// Temp file then rename, since a half-written index reads as an empty one.
    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let json = serde_json::to_vec_pretty(&self.index)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let temp = self.root.join(format!("{}.tmp", super::INDEX_FILE));
        std::fs::write(&temp, &json)?;
        let final_path = self.root.join(super::INDEX_FILE);
        // Windows refuses to rename onto an existing file.
        if final_path.exists() {
            std::fs::remove_file(&final_path)?;
        }
        std::fs::rename(&temp, &final_path)
    }
}
