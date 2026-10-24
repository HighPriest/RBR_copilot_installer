/// The files a package will own, in zip order; directory entries are left out,
/// since an empty directory is nothing to back up, record or delete.
pub fn entries(zip_bytes: &[u8]) -> Result<Vec<String>, super::ZipError> {
    Ok(super::central_directory::read(zip_bytes)?
        .into_iter()
        .filter(|entry| !entry.is_dir)
        .map(|entry| entry.path)
        .collect())
}

/// Extracts into `dest_dir`, overwriting existing files as the legacy installer
/// did, and returns the files written in the form `entries` reports them.
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
        let target = join(dest_dir, &entry.path);
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

/// Joined segment by segment, so the platform never re-parses the string as a path.
fn join(dest: &std::path::Path, relative: &str) -> std::path::PathBuf {
    let mut path = dest.to_path_buf();
    for segment in relative.split('/') {
        path.push(segment);
    }
    path
}
