/// Bytes written so far across every install in the run.
pub struct Progress {
    pub written: u64,
    pub total: u64,
}

/// Which payload to install and what the ledger says about it, passed in
/// because the cache index holds only the newest version, while a restore
/// names an older payload that is still on disk.
pub struct Installing<'a> {
    pub key: &'a str,
    pub sha256: &'a str,
    pub version: &'a str,
    pub exclusive_group: Option<&'a str>,
}

/// Copies out what the package replaces, records it as `installing`, then
/// extracts, so a run that dies midway leaves files the next run knows are ours.
pub fn install_package(
    run: &mut super::run::RunContext,
    what: &Installing,
    files: &[crate::zip::FileEntry],
    point: &str,
    progress: &mut Progress,
) -> Result<(), super::ExecuteError> {
    let key = what.key;
    let payload = std::fs::read(run.cache.payload_path(what.sha256))
        .map_err(|e| super::ExecuteError::extract(key, e))?;
    let archive =
        crate::zip::Archive::open(&payload).map_err(|e| super::ExecuteError::extract(key, e))?;

    let phase = crate::engine::Phase::BackingUp;
    run.watcher
        .phase(phase, key, progress.written, progress.total);
    let mut entry = crate::ledger::LedgerEntry {
        sha256: what.sha256.to_string(),
        version: what.version.to_string(),
        exclusive_group: what.exclusive_group.map(str::to_string),
        state: crate::ledger::EntryState::Installing,
        files: record_priors(run, files, point, &archive)?,
    };
    run.ledger
        .put(key, entry.clone())
        .map_err(super::ExecuteError::ledger)?;

    let phase = crate::engine::Phase::Installing;
    run.watcher
        .phase(phase, key, progress.written, progress.total);
    for file in files {
        let recorded = entry.files.get_mut(&crate::ledger::fold(&file.path));
        // Already these bytes, hashed when they were compared: nothing to write.
        let identical = recorded
            .as_ref()
            .is_some_and(|recorded| recorded.prior == crate::ledger::Prior::Identical);
        if !identical {
            let bytes = archive
                .read(&file.path)
                .map_err(|e| super::ExecuteError::extract(key, e))?;
            super::game_files::write_atomic(
                &crate::relative_path::join(run.game_dir, &file.path),
                &bytes,
            )?;
            if let Some(recorded) = recorded {
                recorded.sha256 = crate::sha256::hex_digest(&bytes);
            }
        }
        progress.written += file.size;
        run.watcher
            .phase(phase, &file.path, progress.written, progress.total);
    }

    entry.state = crate::ledger::EntryState::Installed;
    run.ledger
        .put(key, entry)
        .map_err(super::ExecuteError::ledger)?;
    run.watcher.log(&format!("installed {key}"));
    Ok(())
}

/// The ledger's files for the package, each hash left empty until it is
/// written, except an identical file's, which is already known.
fn record_priors(
    run: &mut super::run::RunContext,
    files: &[crate::zip::FileEntry],
    point: &str,
    archive: &crate::zip::Archive,
) -> Result<std::collections::BTreeMap<String, crate::ledger::LedgerFile>, super::ExecuteError> {
    let mut recorded = std::collections::BTreeMap::new();
    for file in files {
        let (prior, sha256) = preserve_prior(run, point, file, archive)?;
        let ledger_file = crate::ledger::LedgerFile {
            path: file.path.clone(),
            sha256,
            prior,
        };
        recorded.insert(crate::ledger::fold(&file.path), ledger_file);
    }
    Ok(recorded)
}

/// Copies the user's file at this path into the restore point, unless there is
/// none, it is already ours, or it already holds exactly the incoming bytes,
/// whose hash then comes back with it.
fn preserve_prior(
    run: &mut super::run::RunContext,
    point: &str,
    file: &crate::zip::FileEntry,
    archive: &crate::zip::Archive,
) -> Result<(crate::ledger::Prior, String), super::ExecuteError> {
    let target = crate::relative_path::join(run.game_dir, &file.path);
    // An owned file's original was recorded by whichever entry first replaced it.
    if !target.exists() || run.ledger.owns(&file.path) {
        return Ok((crate::ledger::Prior::Absent, String::new()));
    }
    if let (Ok(existing), Ok(incoming)) = (std::fs::read(&target), archive.read(&file.path)) {
        if existing == incoming {
            let sha256 = crate::sha256::hex_digest(&incoming);
            return Ok((crate::ledger::Prior::Identical, sha256));
        }
    }

    let copy = super::restore_point::saved_copy(run.game_dir, point, &file.path);
    super::game_files::copy_file(&target, &copy)?;
    run.watcher
        .log(&format!("saved the existing {}", file.path));
    let prior = crate::ledger::Prior::Backup {
        point: point.to_string(),
    };
    Ok((prior, String::new()))
}
