/// Creates `.pacenotes\restore\<now>\` holding the ledger as it is before the
/// run, which is what lets a restore also remove what was installed afterwards.
pub fn open_restore_point(
    game_dir: &std::path::Path,
    ledger: &crate::ledger::Ledger,
    watcher: &mut dyn crate::engine::Watcher,
) -> Result<String, super::ExecuteError> {
    let name = unused_point_name(
        game_dir,
        &crate::utc_time::file_stamp(crate::utc_time::unix_now()),
    );
    let dir = restore_dir(game_dir, &name);
    std::fs::create_dir_all(dir.join("files"))
        .map_err(|e| super::ExecuteError::io(format!("creating the restore point {name}"), e))?;
    ledger
        .save_snapshot(&snapshot_path(game_dir, &name))
        .map_err(|e| super::ExecuteError::io("recording what is installed", e))?;
    watcher.log(&format!("restore point {name}"));
    Ok(name)
}

/// Where restore points live, under the game's state directory.
pub const RESTORE_DIR: &str = "restore";

pub fn restore_dir(game_dir: &std::path::Path, point: &str) -> std::path::PathBuf {
    game_dir
        .join(crate::ledger::STATE_DIR)
        .join(RESTORE_DIR)
        .join(point)
}

/// The ledger as it was before the run that followed `point`.
pub fn snapshot_path(game_dir: &std::path::Path, point: &str) -> std::path::PathBuf {
    restore_dir(game_dir, point).join("ledger.json")
}

/// `stamp`, or `stamp-2`, `stamp-3`… when that point exists, since two runs
/// can share a second and the second must not overwrite the first's way back;
/// the suffixed name still sorts after the plain one.
fn unused_point_name(game_dir: &std::path::Path, stamp: &str) -> String {
    let free = |name: &str| !restore_dir(game_dir, name).exists();
    if free(stamp) {
        return stamp.to_string();
    }
    // Bounded: a hundred runs in one second is not a real case.
    (2..100)
        .map(|n| format!("{stamp}-{n}"))
        .find(|candidate| free(candidate))
        .unwrap_or_else(|| stamp.to_string())
}

/// Where `point` keeps its copy of the file at `relative`.
pub fn saved_copy(game_dir: &std::path::Path, point: &str, relative: &str) -> std::path::PathBuf {
    crate::relative_path::join(&restore_dir(game_dir, point).join("files"), relative)
}
