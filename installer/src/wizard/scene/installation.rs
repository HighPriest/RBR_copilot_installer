use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// Scene 6: apply the selection to the game folder from the cache. It has no
/// Back, Next or Cancel: every phase writes to the game folder and the engine
/// cannot stop between files, and it presses Next itself when the run ends.
pub unsafe extern "system" fn installation_page(
    dialog: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match message {
        wm::WM_INITDIALOG => {
            crate::wizard::page::adopt_state(dialog, lparam);
            super::progress_page::init_progress(dialog);
            1
        }
        crate::worker::WM_WORKER_PROGRESS => {
            let Some(progress) = crate::worker::take_progress(lparam) else {
                return 0;
            };
            if let Some(state) = crate::wizard::page::state(dialog) {
                if state.is_current_request(wparam) {
                    super::progress_page::show_progress(dialog, state, &progress);
                }
            }
            1
        }
        crate::worker::WM_WORKER_DONE => {
            let Some(message) = crate::worker::take(lparam) else {
                return 0;
            };
            if let (Some(state), crate::worker::Message::Installed(outcome)) =
                (crate::wizard::page::state(dialog), message)
            {
                if state.is_current_request(wparam) {
                    finish(dialog, state, outcome);
                }
            }
            1
        }
        wm::WM_NOTIFY => match crate::wizard::page::notify_code(lparam) {
            Some(ctl::PSN_SETACTIVE) => {
                crate::wizard::page::set_sheet_caption(dialog);
                crate::wizard::page::set_header(dialog, crate::i18n::IDS_S6_TITLE);
                crate::wizard::page::set_buttons(dialog, 0);
                crate::ui::enable(
                    crate::wizard::page::sheet(dialog),
                    crate::ui::IDCANCEL,
                    false,
                );
                start(dialog);
                crate::wizard::page::reply(dialog, 0)
            }
            Some(ctl::PSN_QUERYCANCEL) => super::progress_page::on_cancel(dialog),
            _ => 0,
        },
        _ => 0,
    }
}

/// Everything it installs from is already in the cache, fetched by scene 4b or
/// already there on the other routes.
unsafe fn start(dialog: HWND) {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return;
    };
    if state.install_started {
        return;
    }
    state.install_started = true;
    if state.route == crate::wizard::state::Route::Restore {
        start_restore(dialog, state);
        return;
    }

    for key in state.config.install_keys() {
        super::progress_page::log_line(dialog, state, &format!("selected: {key}"));
    }
    // Asked before anything moves, because the tree ticks rows on the user's behalf.
    if !confirm_removals(dialog, state) {
        finish(dialog, state, crate::worker::Outcome::Cancelled);
        return;
    }

    crate::ui::set_text(
        dialog,
        crate::wizard::control_ids::IDC_S6_PHASE,
        crate::i18n::IDS_S6_CHECKING,
    );
    let request = state.next_request();
    let job = crate::worker::InstallJob {
        base_url: state.base_url.clone(),
        cache_root: state.cache.root().to_path_buf(),
        game_dir: std::path::PathBuf::from(&state.folder),
        game: state
            .game
            .unwrap_or(pacenotes_installer::gamedir::Game::Rbr),
        selection: state.config.install_keys(),
        picks: state.config.picks_by_category(),
        choice_categories: choice_categories(state),
        facts: host_facts(&state.folder),
        cancel: std::sync::Arc::clone(&state.cancel),
    };
    crate::worker::spawn_reporting(dialog, request, move |reporter| {
        crate::worker::run_install(job, reporter)
    });
}

/// Hands the restore the user approved to the worker, its plan unchanged, since
/// the preview they read is that value.
unsafe fn start_restore(dialog: HWND, state: &mut crate::wizard::state::WizardState) {
    let Some(point) = state.restore_to.clone() else {
        finish(dialog, state, crate::worker::Outcome::Cancelled);
        return;
    };
    let plan = &state.restore_plan;
    let lines: Vec<String> = std::iter::once(format!("restoring to {}", point.name))
        .chain(plan.remove.iter().map(|key| format!("to remove: {key}")))
        .chain(
            plan.put_back
                .iter()
                .map(|wanted| format!("to put back: {}", wanted.key)),
        )
        .collect();
    for line in &lines {
        super::progress_page::log_line(dialog, state, line);
    }

    crate::ui::set_text(
        dialog,
        crate::wizard::control_ids::IDC_S6_PHASE,
        crate::i18n::IDS_RESTORE_RUNNING,
    );
    let request = state.next_request();
    let job = crate::worker::RestoreJob {
        cache_root: state.cache.root().to_path_buf(),
        game_dir: std::path::PathBuf::from(&state.folder),
        game: state
            .game
            .unwrap_or(pacenotes_installer::gamedir::Game::Rbr),
        target: point.snapshot,
        plan: state.restore_plan.clone(),
        choice_categories: choice_categories(state),
        facts: host_facts(&state.folder),
    };
    crate::worker::spawn_reporting(dialog, request, move |reporter| {
        crate::worker::run_restore(job, reporter)
    });
}

fn choice_categories(state: &crate::wizard::state::WizardState) -> Vec<String> {
    pacenotes_installer::schema::choice_categories(state.game_id())
        .iter()
        .map(|category| category.to_string())
        .collect()
}

/// What the engine cannot find out for itself, since it stays testable off Windows.
unsafe fn host_facts(folder: &str) -> pacenotes_installer::engine::HostFacts {
    pacenotes_installer::engine::HostFacts {
        free_bytes: free_space(folder),
        game_running: false, // TODO(I9): look for the game's marker process
    }
}

/// Names every installed package the selection no longer holds and asks
/// before removing them; false when the user declines.
unsafe fn confirm_removals(dialog: HWND, state: &mut crate::wizard::state::WizardState) -> bool {
    let selected = state.config.install_keys();
    let going: Vec<String> = pacenotes_installer::ledger::Ledger::open(&state.folder)
        .packages()
        .keys()
        .filter(|key| !selected.contains(key))
        .cloned()
        .collect();
    if going.is_empty() {
        return true;
    }
    let question =
        crate::i18n::load_string_arg(crate::i18n::IDS_S6_CONFIRM_REMOVE, &going.join("\n"));
    if crate::ui::confirm(dialog, &question) {
        return true;
    }
    super::progress_page::log_line(dialog, state, "the user declined to remove anything");
    false
}

/// `None` when it cannot be measured, which makes the engine skip its space check.
unsafe fn free_space(folder: &str) -> Option<u64> {
    let wide = crate::i18n::to_wide(folder);
    let mut free: u64 = 0;
    let measured = windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
        wide.as_ptr(),
        &mut free,
        core::ptr::null_mut(),
        core::ptr::null_mut(),
    );
    (measured != 0).then_some(free)
}

/// Records the outcome, reopens the cache the worker wrote, and moves on to the summary.
unsafe fn finish(
    dialog: HWND,
    state: &mut crate::wizard::state::WizardState,
    outcome: crate::worker::Outcome,
) {
    let line = match &outcome {
        crate::worker::Outcome::Done { applied } => format!(
            "finished: {} installed, {} removed",
            applied.installed.len(),
            applied.uninstalled.len()
        ),
        crate::worker::Outcome::Failed(message) => format!("failed: {message}"),
        crate::worker::Outcome::Cancelled => "cancelled by the user".to_string(),
    };
    super::progress_page::log_line(dialog, state, &line);
    if let crate::worker::Outcome::Done { applied } = &outcome {
        if let Some(point) = &applied.restore_point {
            super::progress_page::log_line(dialog, state, &format!("restore point: {point}"));
        }
    }
    state.outcome = Some(outcome);
    state.cache = pacenotes_installer::cache::Cache::open(state.cache.root().to_path_buf());
    super::progress_page::finish_page(dialog, crate::i18n::IDS_S6_FINISHED, None);
}
