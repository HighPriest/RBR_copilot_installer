/// What a run did, for the summary.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub installed: Vec<String>,
    pub uninstalled: Vec<String>,
    /// Kept rather than deleted, because they could not be put back or were
    /// changed by someone else after we wrote them.
    pub left_alone: Vec<String>,
    /// `None` when the plan had nothing to do.
    pub restore_point: Option<String>,
}

/// What every step of one run works on.
pub struct RunContext<'a> {
    pub game_dir: &'a std::path::Path,
    pub cache: &'a crate::cache::Cache,
    pub ledger: &'a mut crate::ledger::Ledger,
    pub watcher: &'a mut dyn crate::engine::Watcher,
}

/// Carries out `plan` with what `preflight` prepared for it.
pub fn execute(
    plan: &crate::engine::Plan,
    prepared: &crate::engine::Preflight,
    game_dir: &std::path::Path,
    cache: &crate::cache::Cache,
    ledger: &mut crate::ledger::Ledger,
    watcher: &mut dyn crate::engine::Watcher,
) -> Result<Outcome, super::ExecuteError> {
    let mut outcome = Outcome::default();
    if plan.is_empty() {
        return Ok(outcome);
    }
    let point = super::restore_point::open_restore_point(game_dir, ledger, watcher)?;
    outcome.restore_point = Some(point.clone());
    let mut run = RunContext {
        game_dir,
        cache,
        ledger,
        watcher,
    };

    let uninstalls = plan.uninstalls();
    for (index, key) in uninstalls.iter().enumerate() {
        let count = uninstalls.len() as u64;
        run.watcher
            .phase(crate::engine::Phase::Uninstalling, key, index as u64, count);
        super::uninstall_package::uninstall_package(&mut run, key, &mut outcome)?;
    }

    let mut progress = super::install_package::Progress {
        written: 0,
        total: prepared.install_bytes,
    };
    for key in plan.installs() {
        let files = prepared.files.get(key).map_or(&[][..], Vec::as_slice);
        super::install_package::install_package(&mut run, key, files, &point, &mut progress)?;
        outcome.installed.push(key.to_string());
    }
    Ok(outcome)
}
