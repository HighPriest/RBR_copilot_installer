impl super::Cache {
    /// Where a payload with this digest lives, whether or not it is there.
    pub fn payload_path(&self, sha256: &str) -> std::path::PathBuf {
        self.root.join(format!("{sha256}.zip"))
    }

    /// The index can outlive its file, so an install asks this rather than trusting the index.
    pub fn has_payload(&self, sha256: &str) -> bool {
        self.payload_path(sha256).is_file()
    }

    /// Whether `key`'s payload is here to install, not merely named by the index.
    pub fn holds(&self, key: &str) -> bool {
        self.get(key)
            .is_some_and(|entry| self.has_payload(&entry.sha256))
    }

    /// Payloads no index entry refers to, for the I9 sweep, and none unless
    /// the index was really read, or a damaged index would condemn the whole cache.
    pub fn orphans(&self) -> Vec<std::path::PathBuf> {
        if self.state != super::IndexState::Loaded {
            return Vec::new();
        }
        let known = self.known_digests();
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "zip"))
            .filter(|path| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_none_or(|stem| !known.contains(stem))
            })
            .collect()
    }

    /// Deletes what a killed download left half-written, and, given `in_use`
    /// (the digests the ledger and the restore points need), every payload
    /// neither it nor the index names; `None` deletes no payload, for when
    /// what is needed is unknown, and so does an index that was not read.
    pub fn sweep(&self, in_use: Option<&[String]>) -> Swept {
        let mut swept = Swept::default();
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return swept;
        };
        for path in entries.flatten().map(|entry| entry.path()) {
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("");
            let removable = match path.extension().and_then(|extension| extension.to_str()) {
                Some("part") => true,
                Some("zip") => in_use.is_some_and(|in_use| {
                    self.state == super::IndexState::Loaded
                        && !self.names(stem)
                        && !in_use.iter().any(|sha256| sha256 == stem)
                }),
                _ => false,
            };
            let size = std::fs::metadata(&path).map_or(0, |metadata| metadata.len());
            if removable && std::fs::remove_file(&path).is_ok() {
                swept.files += 1;
                swept.bytes += size;
            }
        }
        swept
    }
}

/// What a sweep removed, for the run log.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Swept {
    pub files: usize,
    pub bytes: u64,
}
