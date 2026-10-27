/// `<game dir>\.pacenotes\cache`, beside the ledger, since this installer writes
/// nothing outside the folder it was pointed at.
///
/// Deleting the game takes everything this tool wrote with it, and a folder
/// moved elsewhere brings its downloads along. The cost is accepted: two game
/// folders no longer share a cache, so a package wanted in both is fetched
/// twice, and that second copy keeps the first folder's restore points working
/// after the second folder is gone.
pub fn dir_for(game_dir: &std::path::Path) -> std::path::PathBuf {
    game_dir.join(crate::ledger::STATE_DIR).join("cache")
}
