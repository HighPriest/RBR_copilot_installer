use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// Scene 7: what the run did. Finish is the only way out.
pub unsafe extern "system" fn summary_page(
    dialog: HWND,
    message: u32,
    _wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match message {
        wm::WM_INITDIALOG => {
            crate::wizard::page::adopt_state(dialog, lparam);
            1
        }
        wm::WM_NOTIFY => match crate::wizard::page::notify_code(lparam) {
            Some(ctl::PSN_SETACTIVE) => {
                crate::wizard::page::set_sheet_caption(dialog);
                crate::wizard::page::set_header(dialog, crate::i18n::IDS_S7_TITLE);
                crate::wizard::page::set_buttons(dialog, ctl::PSWIZB_FINISH);
                crate::wizard::page::set_next_text(dialog, crate::i18n::IDS_BTN_FINISH);
                crate::ui::enable(
                    crate::wizard::page::sheet(dialog),
                    crate::ui::IDCANCEL,
                    false,
                );
                if let Some(state) = crate::wizard::page::state(dialog) {
                    show_summary(dialog, state);
                }
                crate::wizard::page::reply(dialog, 0)
            }
            _ => 0,
        },
        _ => 0,
    }
}

/// DESIGN.md: the log collapses on success and opens on failure, and a
/// collapsed log's room goes to the detail block.
unsafe fn show_summary(dialog: HWND, state: &crate::wizard::state::WizardState) {
    crate::ui::set_text_raw(
        dialog,
        crate::wizard::control_ids::IDC_S7_RESULT,
        &result_line(state.outcome.as_ref()),
    );
    crate::ui::set_text_raw(
        dialog,
        crate::wizard::control_ids::IDC_S7_DETAIL,
        &detail_lines(state).join("\r\n"),
    );

    let log = crate::wizard::control_ids::IDC_S7_LOG;
    let failed = matches!(
        state.outcome,
        Some(crate::worker::Outcome::Failed(_)) | None
    );
    crate::ui::set_text_raw(dialog, log, &state.log.text());
    crate::ui::scroll_to_end(dialog, log);
    crate::ui::show(dialog, log, failed);
    if !failed {
        crate::ui::stretch_to_bottom_of(dialog, crate::wizard::control_ids::IDC_S7_DETAIL, log);
    }
}

/// Never claims an install, since backup and install wait for task I7.
fn result_line(outcome: Option<&crate::worker::Outcome>) -> String {
    match outcome {
        Some(crate::worker::Outcome::Done { .. }) => {
            crate::i18n::load_string(crate::i18n::IDS_S7_PENDING_ENGINE)
        }
        Some(crate::worker::Outcome::Failed(message)) => {
            crate::i18n::load_string_arg(crate::i18n::IDS_S7_FAILED, message)
        }
        Some(crate::worker::Outcome::Cancelled) | None => {
            crate::i18n::load_string(crate::i18n::IDS_S7_CANCELLED)
        }
    }
}

fn detail_lines(state: &crate::wizard::state::WizardState) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    if let Some(game) = state.game {
        lines.push(crate::i18n::load_string_arg(
            crate::i18n::IDS_S7_GAME,
            &format!("{} — {}", game.title(), state.folder),
        ));
    }
    for list in &state.config.lists {
        if let Some(option) = list.selected.and_then(|index| list.options.get(index)) {
            let caption =
                crate::i18n::load_string(super::configuration::category_caption(list.category));
            lines.push(format!("{caption} {}", option.label));
        }
    }
    if !state.config.extras.is_empty() {
        let leaves: Vec<&str> = state
            .config
            .extras
            .iter()
            .map(|key| key.rsplit('/').next().unwrap_or(key))
            .collect();
        lines.push(crate::i18n::load_string_arg(
            crate::i18n::IDS_S7_EXTRAS,
            &leaves.join(", "),
        ));
    }
    lines.push(match &state.outcome {
        Some(crate::worker::Outcome::Done { downloaded, bytes }) if *downloaded > 0 => {
            crate::i18n::load_string_arg(
                crate::i18n::IDS_S7_DOWNLOADS,
                &format!(
                    "{downloaded} ({})",
                    pacenotes_installer::size::human(*bytes)
                ),
            )
        }
        _ => crate::i18n::load_string(crate::i18n::IDS_S7_NOTHING_TODO),
    });
    if let Some(path) = state.log.path() {
        lines.push(crate::i18n::load_string_arg(
            crate::i18n::IDS_S7_LOGFILE,
            &path.display().to_string(),
        ));
    }
    lines
}
