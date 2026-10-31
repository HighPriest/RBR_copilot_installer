/// Runs kept per game folder: enough that a re-run or two before asking for
/// help cannot prune the run that failed.
pub const KEEP_LOGS: usize = 5;

/// `<game dir>\.pacenotes\logs`, beside the ledger, so one folder holds
/// everything this installer knows about one game.
pub fn log_dir(game_dir: &std::path::Path) -> std::path::PathBuf {
    game_dir.join(crate::ledger::STATE_DIR).join("logs")
}

/// One file per run, named by its `utc_time::file_stamp`, so reopening the
/// installer to read a run cannot erase it.
pub fn log_path(game_dir: &std::path::Path, stamp: &str) -> std::path::PathBuf {
    log_dir(game_dir).join(format!("install-{stamp}.log"))
}

/// Deletes all but the newest `keep` run logs in `dir`, by name rather than
/// modification time, so a log restored from a backup cannot pass for the
/// newest; failures are ignored, since housekeeping must never stop an install.
pub fn prune(dir: &std::path::Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut logs: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("install-") && name.ends_with(".log"))
        })
        .collect();
    if logs.len() <= keep {
        return;
    }
    logs.sort();
    for old in &logs[..logs.len() - keep] {
        let _ = std::fs::remove_file(old);
    }
}
