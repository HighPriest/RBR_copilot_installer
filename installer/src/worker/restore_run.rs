/// Everything the restore run needs, owned, because it crosses a thread.
///
/// The plan comes from the UI thread, because it is the preview the user
/// approved; recomputing it here could carry out something they were never shown.
pub struct RestoreJob {
    pub cache_root: std::path::PathBuf,
    pub game_dir: std::path::PathBuf,
    pub game: pacenotes_installer::gamedir::Game,
    /// The state being returned to.
    pub target: pacenotes_installer::restore::Snapshot,
    pub plan: pacenotes_installer::restore::Plan,
    pub choice_categories: Vec<String>,
    pub facts: pacenotes_installer::engine::HostFacts,
}

/// The restore scene's run, which reaches no server, since the way back must
/// work when the server does not.
pub fn run_restore(job: RestoreJob, reporter: &super::Reporter) -> super::Message {
    match restore(&job, reporter) {
        Ok(applied) => super::Message::Installed(super::Outcome::Done { applied }),
        Err(message) => super::Message::Installed(super::Outcome::Failed(message)),
    }
}

/// Puts the folder back, then runs the game's configuration step for the picks
/// the restored state implies, since those edits live in a file the packages
/// ship their own defaults for.
fn restore(job: &RestoreJob, reporter: &super::Reporter) -> Result<super::Applied, String> {
    let cache = pacenotes_installer::cache::Cache::open(&job.cache_root);
    let mut watcher = super::apply_selection::PostingWatcher { reporter };
    let prepared = pacenotes_installer::restore::prepare(
        &job.plan,
        &cache,
        job.facts,
        &job.game_dir,
        &mut watcher,
    )
    .map_err(|e| e.to_string())?;

    let mut ledger = pacenotes_installer::ledger::Ledger::open(&job.game_dir);
    let outcome = pacenotes_installer::restore::execute(
        &job.target,
        &job.plan,
        &prepared,
        &job.game_dir,
        &cache,
        &mut ledger,
        &mut watcher,
    )
    .map_err(|e| e.to_string())?;

    // A step rewriting a file it did not need to would look like someone else's edit.
    let configured = if job.plan.is_empty() {
        Vec::new()
    } else {
        let choices: Vec<&str> = job.choice_categories.iter().map(String::as_str).collect();
        let picks = pacenotes_installer::restore::picks(&job.target, &choices);
        super::apply_selection::configure(
            job.game,
            &job.game_dir,
            &picks,
            &mut ledger,
            &mut watcher,
        )?
    };
    Ok(super::Applied {
        installed: outcome.installed,
        uninstalled: outcome.uninstalled,
        left_alone: outcome.left_alone,
        configured,
        overlaps: 0,
        restore_point: outcome.restore_point,
    })
}
