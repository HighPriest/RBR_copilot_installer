use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// Progress bar steps; bytes would overflow `PBM_SETRANGE32`'s i32 past 2 GB.
const PROGRESS_SCALE: u32 = 1000;

/// Scene 6: download what is missing, then (from I7) back up and install.
/// It has no Back or Next: it presses Next itself when the run ends.
pub unsafe extern "system" fn installation_page(
    dialog: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match message {
        wm::WM_INITDIALOG => {
            crate::wizard::page::adopt_state(dialog, lparam);
            wm::SendDlgItemMessageW(
                dialog,
                crate::wizard::control_ids::IDC_S6_PROGRESS as i32,
                ctl::PBM_SETRANGE32,
                0,
                PROGRESS_SCALE as LPARAM,
            );
            1
        }
        crate::worker::WM_WORKER_PROGRESS => {
            let Some(progress) = crate::worker::take_progress(lparam) else {
                return 0;
            };
            if let Some(state) = crate::wizard::page::state(dialog) {
                if state.is_current_request(wparam) {
                    show_progress(dialog, state, &progress);
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
                start(dialog);
                crate::wizard::page::reply(dialog, 0)
            }
            Some(ctl::PSN_QUERYCANCEL) => on_cancel(dialog),
            _ => 0,
        },
        _ => 0,
    }
}

/// Downloads what scene 4 ticked and the cache lacks, which is not the same as the scene 5 picks.
unsafe fn start(dialog: HWND) {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return;
    };
    if state.install_started {
        return;
    }
    state.install_started = true;

    if let Some(game) = state.game {
        let line = format!("game: {} in {}", game.title(), state.folder);
        log_line(dialog, state, &line);
    }
    for key in state.config.install_keys() {
        log_line(dialog, state, &format!("selected: {key}"));
    }

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
        .collect();
    if missing.is_empty() {
        crate::ui::set_text(
            dialog,
            crate::wizard::control_ids::IDC_S6_PHASE,
            crate::i18n::IDS_S6_NOTHING,
        );
        log_line(
            dialog,
            state,
            &crate::i18n::load_string(crate::i18n::IDS_S6_NOTHING),
        );
        let nothing = crate::worker::Outcome::Done {
            downloaded: 0,
            bytes: 0,
        };
        finish(dialog, state, nothing);
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
    let base_url = state.base_url.clone();
    let token = state.token();
    let cache_root = state.cache.root().to_path_buf();
    let cancel = std::sync::Arc::clone(&state.cancel);
    crate::worker::spawn_reporting(dialog, request, move |reporter| {
        crate::worker::run_install(base_url, token, cache_root, missing, cancel, reporter)
    });
}

unsafe fn show_progress(
    dialog: HWND,
    state: &mut crate::wizard::state::WizardState,
    progress: &crate::worker::Progress,
) {
    let phase = match progress.phase {
        crate::worker::Phase::Download => crate::i18n::IDS_S6_DOWNLOADING,
        crate::worker::Phase::Install => crate::i18n::IDS_S6_INSTALLING,
    };
    crate::ui::set_text(dialog, crate::wizard::control_ids::IDC_S6_PHASE, phase);

    let item = if progress.item.is_empty() {
        String::new()
    } else {
        format!(
            "{} — {}",
            progress.item,
            pacenotes_installer::size::human_progress(progress.item_done, progress.item_size)
        )
    };
    crate::ui::set_text_raw(dialog, crate::wizard::control_ids::IDC_S6_ITEM, &item);

    if let Some(position) = progress
        .done
        .saturating_mul(PROGRESS_SCALE as u64)
        .checked_div(progress.total)
    {
        set_progress(dialog, position as WPARAM);
    }
    if let Some(line) = &progress.log {
        log_line(dialog, state, line);
    }
}

/// Records the outcome, reopens the cache the worker wrote, and moves on to the summary.
unsafe fn finish(
    dialog: HWND,
    state: &mut crate::wizard::state::WizardState,
    outcome: crate::worker::Outcome,
) {
    let line = match &outcome {
        crate::worker::Outcome::Done { downloaded, bytes } => format!(
            "finished: {downloaded} package(s) downloaded, {}",
            pacenotes_installer::size::human(*bytes)
        ),
        crate::worker::Outcome::Failed(message) => format!("failed: {message}"),
        crate::worker::Outcome::Cancelled => "cancelled by the user".to_string(),
    };
    log_line(dialog, state, &line);
    state.outcome = Some(outcome);
    state.cache = pacenotes_installer::cache::Cache::open(state.cache.root().to_path_buf());

    crate::ui::set_text(
        dialog,
        crate::wizard::control_ids::IDC_S6_PHASE,
        crate::i18n::IDS_S6_FINISHED,
    );
    crate::ui::set_text_raw(dialog, crate::wizard::control_ids::IDC_S6_ITEM, "");
    set_progress(dialog, PROGRESS_SCALE as WPARAM);
    crate::ui::enable(
        crate::wizard::page::sheet(dialog),
        crate::ui::IDCANCEL,
        false,
    );

    crate::wizard::page::set_buttons(dialog, ctl::PSWIZB_NEXT);
    wm::PostMessageW(
        crate::wizard::page::sheet(dialog),
        ctl::PSM_PRESSBUTTON,
        ctl::PSBTN_NEXT as WPARAM,
        0,
    );
}

/// Always vetoes the close: a confirmed Cancel raises the flag, and the
/// worker's `Cancelled` then walks the wizard on to the summary.
unsafe fn on_cancel(dialog: HWND) -> isize {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return crate::wizard::page::reply(dialog, 0);
    };
    if state.outcome.is_some() {
        return crate::wizard::page::reply(dialog, 0);
    }
    let veto_close = 1;
    if !crate::ui::confirm(
        dialog,
        &crate::i18n::load_string(crate::i18n::IDS_CANCEL_CONFIRM),
    ) {
        return crate::wizard::page::reply(dialog, veto_close);
    }
    state
        .cancel
        .store(true, std::sync::atomic::Ordering::Relaxed);
    crate::ui::set_text(
        dialog,
        crate::wizard::control_ids::IDC_S6_PHASE,
        crate::i18n::IDS_S6_CANCELLING,
    );
    log_line(dialog, state, "stopping at the user's request");
    crate::wizard::page::reply(dialog, veto_close)
}

unsafe fn set_progress(dialog: HWND, position: WPARAM) {
    wm::SendDlgItemMessageW(
        dialog,
        crate::wizard::control_ids::IDC_S6_PROGRESS as i32,
        ctl::PBM_SETPOS,
        position,
        0,
    );
}

/// Appends to the run log and shows its newest line in the pane.
unsafe fn log_line(dialog: HWND, state: &mut crate::wizard::state::WizardState, text: &str) {
    state.log.line(text);
    crate::ui::set_text_raw(
        dialog,
        crate::wizard::control_ids::IDC_S6_LOG,
        &state.log.text(),
    );
    crate::ui::scroll_to_end(dialog, crate::wizard::control_ids::IDC_S6_LOG);
}
