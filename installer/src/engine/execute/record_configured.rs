/// Puts the hashes of the files a game's configuration step edited back into
/// the ledger, or uninstall would read our own edit as someone else's.
///
/// A file no package owns is logged, never adopted: the step cannot know
/// whether something was there before it, and an uninstall could then delete it.
pub fn record_configured(
    game_dir: &std::path::Path,
    files: &[String],
    ledger: &mut crate::ledger::Ledger,
    watcher: &mut dyn crate::engine::Watcher,
) -> Result<(), super::ExecuteError> {
    for path in files {
        let owners: Vec<String> = ledger
            .owners_of(path)
            .into_iter()
            .map(str::to_string)
            .collect();
        if owners.is_empty() {
            watcher.log(&format!(
                "{path} was written by the configuration step and belongs to no \
                 package; it will be left in place if everything is uninstalled"
            ));
            continue;
        }
        let Ok(sha) =
            crate::sha256::hex_digest_of_file(&crate::relative_path::join(game_dir, path))
        else {
            continue;
        };
        for owner in owners {
            let Some(mut entry) = ledger.get(&owner).cloned() else {
                continue;
            };
            if let Some(file) = entry.files.get_mut(&crate::ledger::fold(path)) {
                file.sha256 = sha.clone();
            }
            ledger
                .put(&owner, entry)
                .map_err(super::ExecuteError::ledger)?;
        }
    }
    Ok(())
}
