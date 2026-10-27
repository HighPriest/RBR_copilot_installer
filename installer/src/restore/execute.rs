/// Everything read and checked before the first file moves, as `engine::preflight`
/// does for an install.
#[derive(Debug, Default)]
pub struct Prepared {
    /// Per package to put back, the files its payload holds.
    pub files: std::collections::BTreeMap<String, Vec<crate::zip::FileEntry>>,
    pub install_bytes: u64,
}

/// Finds a missing or damaged payload, an unwritable folder, too little room
/// or a running game while the folder is still untouched.
pub fn prepare(
    plan: &super::Plan,
    cache: &crate::cache::Cache,
    facts: crate::engine::HostFacts,
    game_dir: &std::path::Path,
    watcher: &mut dyn crate::engine::Watcher,
) -> Result<Prepared, crate::engine::PreflightError> {
    if facts.game_running {
        return Err(crate::engine::PreflightError::GameRunning);
    }
    crate::engine::probe_writable(game_dir)?;

    let mut prepared = Prepared::default();
    let count = plan.put_back.len() as u64;
    for (index, wanted) in plan.put_back.iter().enumerate() {
        let phase = crate::engine::Phase::Checking;
        watcher.phase(phase, &wanted.key, index as u64, count);
        let listed = crate::engine::verified_files(cache, &wanted.key, &wanted.sha256)?;
        prepared.install_bytes += listed.iter().map(|file| file.size).sum::<u64>();
        prepared.files.insert(wanted.key.clone(), listed);
    }
    crate::engine::check_space(prepared.install_bytes, facts)?;
    Ok(prepared)
}

/// Carries out a restore, opening a restore point of its own first, so a
/// restore to the wrong point can be undone like any other run.
///
/// The ledger is rewritten even when a put-back fails, so a restore that
/// stopped halfway still describes the folder truthfully.
pub fn execute(
    target: &super::Snapshot,
    plan: &super::Plan,
    prepared: &Prepared,
    game_dir: &std::path::Path,
    cache: &crate::cache::Cache,
    ledger: &mut crate::ledger::Ledger,
    watcher: &mut dyn crate::engine::Watcher,
) -> Result<crate::engine::Outcome, crate::engine::ExecuteError> {
    let mut outcome = crate::engine::Outcome::default();
    if plan.is_empty() {
        return Ok(outcome);
    }
    let live = ledger.packages().clone();
    let point = crate::engine::open_restore_point(game_dir, ledger, watcher)?;
    outcome.restore_point = Some(point.clone());
    let run = &mut crate::engine::RunContext {
        game_dir,
        cache,
        ledger,
        watcher,
    };

    let count = plan.remove.len() as u64;
    for (index, key) in plan.remove.iter().enumerate() {
        let phase = crate::engine::Phase::Uninstalling;
        run.watcher.phase(phase, key, index as u64, count);
        crate::engine::uninstall_package(run, key, &mut outcome)?;
    }

    let put_back = put_back_all(target, plan, prepared, run, &point, &mut outcome);
    let failed: Vec<String> = plan
        .put_back
        .iter()
        .map(|wanted| wanted.key.clone())
        .filter(|key| !outcome.installed.contains(key))
        .collect();
    run.ledger
        .replace_all(super::ledger_after(target, &live, plan, &failed))
        .map_err(crate::engine::ExecuteError::ledger)?;
    put_back?;

    for wanted in &plan.unavailable {
        run.watcher.log(&format!(
            "{} could not be put back: the download it needs is no longer in the cache, \
             so what is installed was left alone",
            wanted.key
        ));
    }
    Ok(outcome)
}

/// Installs each payload the snapshot names, stopping at the first failure.
fn put_back_all(
    target: &super::Snapshot,
    plan: &super::Plan,
    prepared: &Prepared,
    run: &mut crate::engine::RunContext,
    point: &str,
    outcome: &mut crate::engine::Outcome,
) -> Result<(), crate::engine::ExecuteError> {
    let mut progress = crate::engine::Progress {
        written: 0,
        total: prepared.install_bytes,
    };
    for wanted in &plan.put_back {
        let files = prepared
            .files
            .get(&wanted.key)
            .map_or(&[][..], Vec::as_slice);
        let group = target
            .get(&wanted.key)
            .and_then(|entry| entry.exclusive_group.as_deref());
        let what = crate::engine::Installing {
            key: &wanted.key,
            sha256: &wanted.sha256,
            version: &wanted.version,
            exclusive_group: group,
        };
        crate::engine::install_package(run, &what, files, point, &mut progress)?;
        outcome.installed.push(wanted.key.clone());
    }
    Ok(())
}
