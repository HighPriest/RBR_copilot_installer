/// Bumped when the shape changes in a way an older installer must not read.
const INDEX_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Index {
    /// Absent in the first builds' files.
    #[serde(default)]
    version: u32,
    #[serde(default)]
    packages: std::collections::BTreeMap<String, crate::tree::CachedPackage>,
}

/// How `cache.json` was found. Only `Missing` means the payloads on disk are
/// really unreferenced; after `Unreadable` every one of them would look orphaned.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IndexState {
    Missing,
    /// Unparseable, or written by a newer build.
    Unreadable,
    Loaded,
}

pub struct Cache {
    pub(super) root: std::path::PathBuf,
    index: Index,
    pub(super) state: IndexState,
}

impl Cache {
    /// An index that is missing or cannot be read opens as an empty cache and
    /// leaves the payloads on disk; `index_state` says which happened.
    pub fn open(root: impl Into<std::path::PathBuf>) -> Cache {
        let root = root.into();
        let (index, state) = match std::fs::read(root.join(super::INDEX_FILE)) {
            Err(_) => (Index::default(), IndexState::Missing),
            Ok(bytes) => match serde_json::from_slice::<Index>(&bytes) {
                Ok(index) if index.version <= INDEX_VERSION => (index, IndexState::Loaded),
                Ok(_) | Err(_) => (Index::default(), IndexState::Unreadable),
            },
        };
        Cache { root, index, state }
    }

    pub fn index_state(&self) -> IndexState {
        self.state
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

    /// Whether an index entry names this payload.
    pub(super) fn names(&self, sha256: &str) -> bool {
        self.index
            .packages
            .values()
            .any(|entry| entry.sha256 == sha256)
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

    /// A temp file renamed over the old index, which `std::fs::rename` replaces
    /// in one step, so a crash never leaves the cache without one.
    pub fn save(&mut self) -> std::io::Result<()> {
        self.index.version = INDEX_VERSION;
        std::fs::create_dir_all(&self.root)?;
        let json = serde_json::to_vec_pretty(&self.index)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let temp = self.root.join(format!("{}.tmp", super::INDEX_FILE));
        std::fs::write(&temp, &json)?;
        std::fs::rename(&temp, self.root.join(super::INDEX_FILE))?;
        self.state = IndexState::Loaded;
        Ok(())
    }
}
