impl super::Cache {
    /// Where a payload with this digest lives, whether or not it is there.
    pub fn payload_path(&self, sha256: &str) -> std::path::PathBuf {
        self.root.join(format!("{sha256}.zip"))
    }

    /// The index can outlive its file, so an install asks this rather than trusting the index.
    pub fn has_payload(&self, sha256: &str) -> bool {
        self.payload_path(sha256).is_file()
    }

    /// Payloads no index entry refers to, for the I9 sweep.
    pub fn orphans(&self) -> Vec<std::path::PathBuf> {
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
}
