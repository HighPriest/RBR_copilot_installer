use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// Scene 4b: fetch what scene 4 ticked into the cache, touching nothing in the
/// game folder, so the questions come after the transfers. Cancel is the only
/// button, honoured here because a half-finished download costs nothing; the
/// page moves on by itself.
pub unsafe extern "system" fn fetching_page(
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
            if let (Some(state), crate::worker::Message::Downloaded(fetched)) =
                (crate::wizard::page::state(dialog), message)
            {
                if state.is_current_request(wparam) {
                    finish(dialog, state, fetched);
                }
            }
            1
        }
        wm::WM_NOTIFY => match crate::wizard::page::notify_code(lparam) {
            Some(ctl::PSN_SETACTIVE) => {
                crate::wizard::page::set_sheet_caption(dialog);
                crate::wizard::page::set_header(dialog, crate::i18n::IDS_S4B_TITLE);
                crate::wizard::page::set_buttons(dialog, 0);
                start(dialog);
                crate::wizard::page::reply(dialog, 0)
            }
            Some(ctl::PSN_QUERYCANCEL) => super::progress_page::on_cancel(dialog),
            _ => 0,
        },
        _ => 0,
    }
}

/// Everything ticked that is not already here, which is the scene 4 selection,
/// since the scene 5 picks are not made yet.
unsafe fn start(dialog: HWND) {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return;
    };
    if state.download_started {
        return;
    }
    state.download_started = true;

    let missing: Vec<pacenotes_shared::PackageInfo> = state
        .tree
        .checked_needing_download()
        .iter()
        .filter_map(|key| {
            state
                .packages
                .iter()
                .find(|package| &package.download_path() == key)
                .cloned()
        })
        // The tree's states date from scene 4's first visit, and a selection
        // sent back here has had a transfer since.
        .filter(|package| {
            let cached = state.cache.get(&package.download_path());
            !cached.is_some_and(|entry| entry.sha256 == package.sha256)
                || !state.cache.has_payload(&package.sha256)
        })
        .collect();
    if missing.is_empty() {
        let nothing = crate::i18n::load_string(crate::i18n::IDS_S6_NOTHING);
        super::progress_page::log_line(dialog, state, &nothing);
        let done = crate::worker::Fetched::Done {
            downloaded: 0,
            bytes: 0,
            failed: Vec::new(),
        };
        finish(dialog, state, done);
        return;
    }

    crate::ui::set_text(
        dialog,
        crate::wizard::control_ids::IDC_S6_PHASE,
        crate::i18n::IDS_S6_DOWNLOADING,
    );
    crate::ui::enable(
        crate::wizard::page::sheet(dialog),
        crate::ui::IDCANCEL,
        true,
    );
    let request = state.next_request();
    let job = crate::worker::DownloadJob {
        base_url: state.base_url.clone(),
        token: state.token(),
        cache_root: state.cache.root().to_path_buf(),
        downloads: missing,
        cancel: std::sync::Arc::clone(&state.cancel),
    };
    crate::worker::spawn_reporting(dialog, request, move |reporter| {
        crate::worker::run_download(job, reporter)
    });
}

/// The questions come next, without what never arrived; when nothing did, or
/// the run was cancelled, it ends at the summary, by page id, since nothing
/// new could honestly be installed. When a category is left with nothing to
/// pick from, it is back to scene 4 for another selection.
unsafe fn finish(
    dialog: HWND,
    state: &mut crate::wizard::state::WizardState,
    fetched: crate::worker::Fetched,
) {
    match fetched {
        crate::worker::Fetched::Done {
            downloaded,
            bytes,
            failed,
        } => {
            // Added up, since a selection sent back from here comes through again.
            state.downloaded += downloaded;
            state.downloaded_bytes += bytes;
            if downloaded > 0 {
                let line = format!(
                    "downloaded {downloaded} package(s), {}",
                    pacenotes_installer::size::human(bytes)
                );
                super::progress_page::log_line(dialog, state, &line);
            }
            // The worker wrote the index through its own handle, and scene 5 and the install read it.
            state.cache = pacenotes_installer::cache::Cache::open(state.cache.root().to_path_buf());
            leave_out(dialog, state, failed);
            if downloaded == 0 && !state.not_downloaded.is_empty() {
                let nothing = crate::i18n::load_string(crate::i18n::IDS_S7_NONE_DOWNLOADED);
                end_run_early(dialog, state, crate::worker::Outcome::Failed(nothing));
                return;
            }
            if !super::configuration::has_something_to_pick(dialog, state) {
                back_to_selection(dialog, state);
                return;
            }
            super::progress_page::finish_page(dialog, crate::i18n::IDS_S4B_DONE, None);
        }
        crate::worker::Fetched::Cancelled => {
            super::progress_page::log_line(dialog, state, "cancelled by the user");
            end_run_early(dialog, state, crate::worker::Outcome::Cancelled);
        }
    }
}

/// Logs each package that never arrived and unticks it, unless the cache holds
/// a copy: a failed UPDATE keeps the installed version, which an untick would
/// have the plan uninstall.
unsafe fn leave_out(
    dialog: HWND,
    state: &mut crate::wizard::state::WizardState,
    failed: Vec<crate::worker::NotDownloaded>,
) {
    for failure in &failed {
        let line = format!("not downloaded: {} — {}", failure.key, failure.reason);
        super::progress_page::log_line(dialog, state, &line);
        if state.cache.get(&failure.key).is_none() {
            state.tree.uncheck_key(&failure.key);
        }
    }
    state.not_downloaded = failed;
}

/// Scene 4 again, with what never arrived unticked there, and a fresh transfer
/// once the user sends a new selection.
unsafe fn back_to_selection(dialog: HWND, state: &mut crate::wizard::state::WizardState) {
    super::progress_page::log_line(
        dialog,
        state,
        "nothing to configure yet: back to the selection",
    );
    state.download_started = false;
    super::progress_page::set_progress(dialog, 0);
    let sheet = crate::wizard::page::sheet(dialog);
    let selection = crate::wizard::control_ids::IDD_S4_DOWNLOAD;
    wm::PostMessageW(sheet, ctl::PSM_SETCURSELID, 0, selection as LPARAM);
}

unsafe fn end_run_early(
    dialog: HWND,
    state: &mut crate::wizard::state::WizardState,
    outcome: crate::worker::Outcome,
) {
    state.outcome = Some(outcome);
    let summary = crate::wizard::control_ids::IDD_S7_SUMMARY;
    super::progress_page::finish_page(dialog, crate::i18n::IDS_S6_FINISHED, Some(summary));
}
