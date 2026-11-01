/// A package whose central directory has been read once. Finding each file
/// by walking the directory again cost an install of n files n² entry
/// parses: 8.5 s of a 12 s install at 3 000 files.
pub struct Archive<'a> {
    zip: &'a [u8],
    /// Files only, by folded path, because a ledger may hold a path folded.
    entries: std::collections::BTreeMap<String, super::central_directory::Entry>,
}

impl<'a> Archive<'a> {
    pub fn open(zip: &'a [u8]) -> Result<Archive<'a>, super::ZipError> {
        let mut entries = std::collections::BTreeMap::new();
        for entry in super::central_directory::read(zip)? {
            // The first of two names that fold alike wins, as a walk would find it.
            if !entry.is_dir {
                entries.entry(entry.path.to_lowercase()).or_insert(entry);
            }
        }
        Ok(Archive { zip, entries })
    }

    /// One file's bytes, by a path `entries` reported, matched case-insensitively.
    pub fn read(&self, path: &str) -> Result<Vec<u8>, super::ZipError> {
        let entry = self
            .entries
            .get(&path.to_lowercase())
            .ok_or_else(|| super::ZipError::new(format!("{path} is not in this package")))?;
        super::entry_data::read(self.zip, entry)
    }
}
