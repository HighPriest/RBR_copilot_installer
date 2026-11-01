/// One file a package will install.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    /// Relative, `/`-separated, validated.
    pub path: String,
    /// Uncompressed, so the pre-flight can tell whether an install fits without extracting it.
    pub size: u64,
}

/// The files a package will own, in zip order; directory entries are left out,
/// since an empty directory is nothing to back up, record or delete.
pub fn entries(zip_bytes: &[u8]) -> Result<Vec<FileEntry>, super::ZipError> {
    Ok(super::central_directory::read(zip_bytes)?
        .into_iter()
        .filter(|entry| !entry.is_dir)
        .map(|entry| FileEntry {
            path: entry.path,
            size: entry.uncompressed as u64,
        })
        .collect())
}

pub fn entry_paths(zip_bytes: &[u8]) -> Result<Vec<String>, super::ZipError> {
    Ok(entries(zip_bytes)?
        .into_iter()
        .map(|entry| entry.path)
        .collect())
}

/// One file's bytes, matched case-insensitively. Reading several from one
/// package goes through `Archive`, which reads the directory only once.
pub fn read_entry(zip_bytes: &[u8], path: &str) -> Result<Vec<u8>, super::ZipError> {
    super::Archive::open(zip_bytes)?.read(path)
}

/// Extracts into `dest_dir`, overwriting existing files as the legacy installer
/// did, and returns the files written in the form `entry_paths` reports them.
///
/// Every name is validated before anything is written, so a hostile entry
/// leaves the target untouched.
pub fn extract(
    zip_bytes: &[u8],
    dest_dir: &std::path::Path,
) -> Result<Vec<String>, super::ZipError> {
    let entries = super::central_directory::read(zip_bytes)?;
    create_dir(dest_dir)?;

    let mut written = Vec::new();
    for entry in &entries {
        let target = crate::relative_path::join(dest_dir, &entry.path);
        if entry.is_dir {
            create_dir(&target)?;
            continue;
        }
        // .NET's zips omit some parent directories as entries of their own.
        if let Some(parent) = target.parent() {
            create_dir(parent)?;
        }
        let data = super::entry_data::read(zip_bytes, entry)?;
        std::fs::write(&target, &data)
            .map_err(|e| super::ZipError::new(format!("cannot write {}: {e}", target.display())))?;
        written.push(entry.path.clone());
    }
    Ok(written)
}

fn create_dir(path: &std::path::Path) -> Result<(), super::ZipError> {
    std::fs::create_dir_all(path)
        .map_err(|e| super::ZipError::new(format!("cannot create {}: {e}", path.display())))
}
