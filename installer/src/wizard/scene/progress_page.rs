//! What scenes 4b and 6 share: they are the same screen at two moments of the
//! run, with the same control ids.

use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// Progress bar steps; bytes would overflow `PBM_SETRANGE32`'s i32 past 2 GB.
pub const PROGRESS_SCALE: u32 = 1000;

pub unsafe fn init_progress(dialog: HWND) {
    wm::SendDlgItemMessageW(
        dialog,
        crate::wizard::control_ids::IDC_S6_PROGRESS as i32,
        ctl::PBM_SETRANGE32,
        0,
        PROGRESS_SCALE as LPARAM,
    );
}

pub unsafe fn set_progress(dialog: HWND, position: u32) {
    wm::SendDlgItemMessageW(
        dialog,
        crate::wizard::control_ids::IDC_S6_PROGRESS as i32,
        ctl::PBM_SETPOS,
        position as WPARAM,
        0,
    );
}

pub unsafe fn show_progress(
    dialog: HWND,
    state: &mut crate::wizard::state::WizardState,
    progress: &crate::worker::Progress,
) {
    let phase = match progress.phase {
        crate::worker::Phase::Download => crate::i18n::IDS_S6_DOWNLOADING,
        crate::worker::Phase::Checking => crate::i18n::IDS_S6_CHECKING,
        crate::worker::Phase::Uninstall => crate::i18n::IDS_S6_UNINSTALLING,
        crate::worker::Phase::Backup => crate::i18n::IDS_S6_BACKUP,
        crate::worker::Phase::Install => crate::i18n::IDS_S6_INSTALLING,
        crate::worker::Phase::Configure => crate::i18n::IDS_S6_CONFIGURING,
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
        set_progress(dialog, position as u32);
    }
    if let Some(line) = &progress.log {
        log_line(dialog, state, line);
    }
}

/// Always vetoes the close: a confirmed Cancel raises the flag, and the
/// worker's `Cancelled` then walks the wizard on to the summary. Scene 6
/// disables the button, since the engine cannot stop between files.
pub unsafe fn on_cancel(dialog: HWND) -> isize {
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

/// Shows the finished run's state, then has the sheet move to `next` by pressing
/// Next, or by page id when the next page is not the one after this.
pub unsafe fn finish_page(dialog: HWND, phase: u16, next: Option<u16>) {
    crate::ui::set_text(dialog, crate::wizard::control_ids::IDC_S6_PHASE, phase);
    crate::ui::set_text_raw(dialog, crate::wizard::control_ids::IDC_S6_ITEM, "");
    set_progress(dialog, PROGRESS_SCALE);
    let sheet = crate::wizard::page::sheet(dialog);
    crate::ui::enable(sheet, crate::ui::IDCANCEL, false);
    crate::wizard::page::set_buttons(dialog, ctl::PSWIZB_NEXT);
    match next {
        None => wm::PostMessageW(sheet, ctl::PSM_PRESSBUTTON, ctl::PSBTN_NEXT as WPARAM, 0),
        Some(page) => wm::PostMessageW(sheet, ctl::PSM_SETCURSELID, 0, page as LPARAM),
    };
}

/// Appends to the run log and adds the line to the pane, where the page has
/// one; `show_log` has brought the pane up to date when the page appeared.
pub unsafe fn log_line(dialog: HWND, state: &mut crate::wizard::state::WizardState, text: &str) {
    state.log.line(text);
    crate::ui::append_line(dialog, crate::wizard::control_ids::IDC_S6_LOG, text);
}

/// The whole log so far, for a page coming into view: lines are logged from
/// pages that have no pane, so this page has not seen them.
pub unsafe fn show_log(dialog: HWND, state: &crate::wizard::state::WizardState) {
    crate::ui::set_text_raw(
        dialog,
        crate::wizard::control_ids::IDC_S6_LOG,
        &state.log.text(),
    );
    crate::ui::scroll_to_end(dialog, crate::wizard::control_ids::IDC_S6_LOG);
}
