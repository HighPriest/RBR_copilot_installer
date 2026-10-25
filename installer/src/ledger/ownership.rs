impl super::Ledger {
    pub fn is_empty(&self) -> bool {
        self.document.packages.is_empty()
    }

    pub fn len(&self) -> usize {
        self.document.packages.len()
    }

    /// By `game/directory/name`.
    pub fn packages(&self) -> &std::collections::BTreeMap<String, super::LedgerEntry> {
        &self.document.packages
    }

    pub fn get(&self, key: &str) -> Option<&super::LedgerEntry> {
        self.document.packages.get(key)
    }

    /// Entries a dead run left mid-install, to deal with before planning anything.
    pub fn unfinished(&self) -> Vec<String> {
        self.document
            .packages
            .iter()
            .filter(|(_, entry)| entry.state == super::EntryState::Installing)
            .map(|(key, _)| key.clone())
            .collect()
    }

    /// Compared folded, since two packages spelling a shared path differently own one file.
    pub fn owners_of(&self, path: &str) -> Vec<&str> {
        let key = fold(path);
        self.document
            .packages
            .iter()
            .filter(|(_, entry)| entry.files.contains_key(&key))
            .map(|(package, _)| package.as_str())
            .collect()
    }

    /// A file nothing owns was the user's, and is backed up before it is overwritten.
    pub fn owns(&self, path: &str) -> bool {
        !self.owners_of(path).is_empty()
    }
}

/// The key a path is stored and compared under: lowercase, as the filesystem
/// compares names, though not NTFS's exact table.
pub fn fold(path: &str) -> String {
    path.to_lowercase()
}
