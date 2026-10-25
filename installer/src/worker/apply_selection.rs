/// Plans against the ledger and carries the plan out, once everything selected
/// is in the cache, then runs the game's own configuration step.
pub fn apply_selection(
    job: &super::InstallJob,
    cache: &pacenotes_installer::cache::Cache,
    reporter: &super::Reporter,
) -> Result<super::Applied, String> {
    let mut ledger = pacenotes_installer::ledger::Ledger::open(&job.game_dir);
    let available = cache
        .entries()
        .into_iter()
        .map(|entry| (entry.key(), entry))
        .collect();
    let choice_categories: Vec<&str> = job.choice_categories.iter().map(String::as_str).collect();
    let plan = pacenotes_installer::engine::plan(&pacenotes_installer::engine::PlanInput {
        game: job.game.id(),
        selection: &job.selection,
        choice_categories: &choice_categories,
        installed: ledger.packages(),
        available: &available,
    })
    .map_err(|e| e.to_string())?;

    let mut watcher = PostingWatcher { reporter };
    let prepared = pacenotes_installer::engine::preflight(
        &plan,
        &job.game_dir,
        cache,
        job.facts,
        &mut watcher,
    )
    .map_err(|e| e.to_string())?;
    let overlaps = log_overlaps(&plan, &prepared, reporter);

    let outcome = pacenotes_installer::engine::execute(
        &plan,
        &prepared,
        &job.game_dir,
        cache,
        &mut ledger,
        &mut watcher,
    )
    .map_err(|e| e.to_string())?;

    let configured = if plan.is_empty() {
        Vec::new()
    } else {
        configure(job, reporter)
    };
    Ok(super::Applied {
        installed: outcome.installed,
        uninstalled: outcome.uninstalled,
        left_alone: outcome.left_alone,
        configured,
        overlaps,
        restore_point: outcome.restore_point,
    })
}

/// The later package wins an overlap, so it is logged rather than refused;
/// returns how many there were.
fn log_overlaps(
    plan: &pacenotes_installer::engine::Plan,
    prepared: &pacenotes_installer::engine::Preflight,
    reporter: &super::Reporter,
) -> usize {
    let install_order: Vec<String> = plan.installs().iter().map(|key| key.to_string()).collect();
    let overlaps = pacenotes_installer::engine::overlaps(&install_order, &prepared.paths());
    for overlap in &overlaps {
        reporter.log(format!(
            "{} is written by more than one package ({}); the last one wins",
            overlap.path,
            overlap.packages.join(", ")
        ));
    }
    overlaps.len()
}

/// Returns the files the step wrote; a failure is logged, not fatal, since
/// the packages are installed and a missing plugin is fixed elsewhere.
fn configure(job: &super::InstallJob, reporter: &super::Reporter) -> Vec<String> {
    reporter.send(super::Progress {
        phase: super::Phase::Configure,
        item: String::new(),
        item_done: 0,
        item_size: 0,
        done: 0,
        total: 0,
        log: None,
    });
    match pacenotes_installer::install::configure(job.game, &job.game_dir, &job.picks) {
        Ok(configured) => {
            for note in configured.notes {
                reporter.log(note);
            }
            configured.files
        }
        Err(e) => {
            reporter.log(format!("configuration skipped — {e}"));
            Vec::new()
        }
    }
}

/// Posts the engine's progress to scene 6.
struct PostingWatcher<'a> {
    reporter: &'a super::Reporter,
}

impl pacenotes_installer::engine::Watcher for PostingWatcher<'_> {
    fn phase(
        &mut self,
        phase: pacenotes_installer::engine::Phase,
        item: &str,
        done: u64,
        total: u64,
    ) {
        self.reporter.send(super::Progress {
            phase: scene_phase(phase),
            item: item.to_string(),
            item_done: 0,
            item_size: 0,
            done,
            total,
            log: None,
        });
    }

    fn log(&mut self, line: &str) {
        self.reporter.log(line.to_string());
    }
}

/// The library's engine knows nothing of the UI, so its phases are mapped here.
fn scene_phase(phase: pacenotes_installer::engine::Phase) -> super::Phase {
    match phase {
        pacenotes_installer::engine::Phase::Checking => super::Phase::Checking,
        pacenotes_installer::engine::Phase::Uninstalling => super::Phase::Uninstall,
        pacenotes_installer::engine::Phase::BackingUp => super::Phase::Backup,
        pacenotes_installer::engine::Phase::Installing => super::Phase::Install,
    }
}
