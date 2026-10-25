/// Through a temp file renamed into place, since a truncated file would be
/// loaded by the game and then hash as someone else's change.
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<(), super::ExecuteError> {
    create_parent(path)?;
    let temp = path.with_extension("pacenotes-part");
    let writing = || format!("writing {}", path.display());
    std::fs::write(&temp, bytes).map_err(|e| super::ExecuteError::io(writing(), e))?;
    std::fs::rename(&temp, path).map_err(|e| super::ExecuteError::io(writing(), e))
}

pub fn copy_file(from: &std::path::Path, to: &std::path::Path) -> Result<(), super::ExecuteError> {
    create_parent(to)?;
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| super::ExecuteError::io(format!("copying {}", from.display()), e))
}

pub fn remove_file(path: &std::path::Path) -> Result<(), super::ExecuteError> {
    std::fs::remove_file(path)
        .map_err(|e| super::ExecuteError::io(format!("removing {}", path.display()), e))
}

fn create_parent(path: &std::path::Path) -> Result<(), super::ExecuteError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent)
        .map_err(|e| super::ExecuteError::io(format!("creating {}", parent.display()), e))
}
