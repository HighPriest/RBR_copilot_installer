/// Removes one package, putting back what each of its files replaced.
pub fn uninstall_package(
    run: &mut super::run::RunContext,
    key: &str,
    outcome: &mut super::Outcome,
) -> Result<(), super::ExecuteError> {
    let Some(entry) = run.ledger.get(key).cloned() else {
        return Ok(());
    };
    // Only a write-ahead entry takes identical originals from the payload (see
    // `put_back`), and it reads the payload once, not once for each of them.
    let identical =
        |file: &crate::ledger::LedgerFile| file.prior == crate::ledger::Prior::Identical;
    let installed = entry.state == crate::ledger::EntryState::Installed;
    let payload = if !installed && entry.files.values().any(identical) {
        std::fs::read(run.cache.payload_path(&entry.sha256)).ok()
    } else {
        None
    };
    let archive = payload
        .as_deref()
        .and_then(|payload| crate::zip::Archive::open(payload).ok());
    for file in entry.files.values() {
        if let Some(reason) = put_back(run, key, &entry, file, archive.as_ref())? {
            run.watcher
                .log(&format!("{} {reason}; left alone", file.path));
            outcome.left_alone.push(file.path.clone());
        }
    }
    prune_empty_dirs(run.game_dir, &entry);
    run.ledger
        .remove(key)
        .map_err(super::ExecuteError::ledger)?;
    outcome.uninstalled.push(key.to_string());
    run.watcher.log(&format!("removed {key}"));
    Ok(())
}

/// Restores what was at the file's path before `key` wrote it, or returns why
/// the file has to stay as it is.
fn put_back(
    run: &mut super::run::RunContext,
    key: &str,
    entry: &crate::ledger::LedgerEntry,
    file: &crate::ledger::LedgerFile,
    payload: Option<&crate::zip::Archive>,
) -> Result<Option<String>, super::ExecuteError> {
    let target = crate::relative_path::join(run.game_dir, &file.path);
    if !target.exists() {
        return Ok(None);
    }
    let others: Vec<&str> = run
        .ledger
        .owners_of(&file.path)
        .into_iter()
        .filter(|owner| *owner != key)
        .collect();
    if !others.is_empty() {
        // It keeps the last writer's bytes, which only this line reveals.
        let line = format!(
            "{} is still used by {}; left in place",
            file.path,
            others.join(", ")
        );
        run.watcher.log(&line);
        return Ok(None);
    }
    // A write-ahead entry never got its per-file hashes, and its files were ours either way.
    if entry.state == crate::ledger::EntryState::Installed {
        match crate::sha256::hex_digest_of_file(&target) {
            Ok(found) if found == file.sha256 => {}
            Ok(_) => return Ok(Some("has changed since it was installed".into())),
            Err(e) => return Ok(Some(format!("could not be read ({e})"))),
        }
    }

    match &file.prior {
        crate::ledger::Prior::Absent => super::game_files::remove_file(&target)?,
        crate::ledger::Prior::Backup { point } => {
            let copy = super::restore_point::saved_copy(run.game_dir, point, &file.path);
            if !copy.is_file() {
                return Ok(Some(
                    "cannot be put back (its saved copy is missing)".into(),
                ));
            }
            super::game_files::copy_file(&copy, &target)?;
        }
        // The hash above proved the file holds what was written, which for an
        // identical one is what was here before: it is the original already.
        crate::ledger::Prior::Identical if entry.state == crate::ledger::EntryState::Installed => {}
        crate::ledger::Prior::Identical => {
            let original = payload.and_then(|archive| archive.read(&file.path).ok());
            let Some(bytes) = original else {
                return Ok(Some(
                    "was already on this computer before it was installed, and the package \
                     it came from is no longer cached"
                        .into(),
                ));
            };
            super::game_files::write_atomic(&target, &bytes)?;
        }
    }
    Ok(None)
}

/// Removes the directories the uninstall emptied, deepest first, up to but not
/// including the game folder.
fn prune_empty_dirs(game_dir: &std::path::Path, entry: &crate::ledger::LedgerEntry) {
    let mut dirs: Vec<std::path::PathBuf> = entry
        .files
        .values()
        .filter_map(|file| {
            crate::relative_path::join(game_dir, &file.path)
                .parent()
                .map(std::path::Path::to_path_buf)
        })
        .collect();
    dirs.sort();
    dirs.dedup();
    dirs.sort_by_key(|dir| std::cmp::Reverse(dir.components().count()));
    for dir in dirs {
        let mut at = dir.as_path();
        while at.starts_with(game_dir) && at != game_dir {
            // Fails on a directory that is not empty, which is where pruning stops.
            if std::fs::remove_dir(at).is_err() {
                break;
            }
            match at.parent() {
                Some(parent) => at = parent,
                None => break,
            }
        }
    }
}
