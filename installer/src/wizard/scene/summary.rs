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
        &result_line(state),
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

/// A restore says what it went back to, since "it worked" does not answer
/// "did I get yesterday back".
fn result_line(state: &crate::wizard::state::WizardState) -> String {
    match state.outcome.as_ref() {
        Some(crate::worker::Outcome::Done { .. })
            if state.route == crate::wizard::state::Route::Restore =>
        {
            let when = state
                .restore_to
                .as_ref()
                .map(|point| pacenotes_installer::restore::human_time(&point.name))
                .unwrap_or_default();
            crate::i18n::load_string_arg(crate::i18n::IDS_S7_RESTORED, &when)
        }
        Some(crate::worker::Outcome::Done { .. }) => {
            crate::i18n::load_string(crate::i18n::IDS_S7_OK)
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
    // From the state, since the transfers happened screens ago, or not at all on the Configure route.
    lines.push(if state.downloaded > 0 {
        crate::i18n::load_string_arg(
            crate::i18n::IDS_S7_DOWNLOADS,
            &format!(
                "{} ({})",
                state.downloaded,
                pacenotes_installer::size::human(state.downloaded_bytes)
            ),
        )
    } else {
        crate::i18n::load_string(crate::i18n::IDS_S7_NOTHING_TODO)
    });
    if let Some(crate::worker::Outcome::Done { applied, .. }) = &state.outcome {
        lines.extend(applied_lines(applied));
    }
    if let Some(path) = state.log.path() {
        lines.push(crate::i18n::load_string_arg(
            crate::i18n::IDS_S7_LOGFILE,
            &path.display().to_string(),
        ));
    }
    lines
}

/// What changed in the game folder, including the files left alone, so a
/// user's own edits surviving does not read as the install failing.
fn applied_lines(applied: &crate::worker::Applied) -> Vec<String> {
    let mut lines = vec![crate::i18n::load_string_arg(
        crate::i18n::IDS_S7_CHANGED,
        &format!(
            "{} / {}",
            applied.installed.len(),
            applied.uninstalled.len()
        ),
    )];
    if !applied.left_alone.is_empty() {
        lines.push(crate::i18n::load_string_arg(
            crate::i18n::IDS_S7_LEFT_ALONE,
            &applied.left_alone.join(", "),
        ));
    }
    if let Some(point) = &applied.restore_point {
        lines.push(crate::i18n::load_string_arg(
            crate::i18n::IDS_S7_RESTORE_POINT,
            point,
        ));
    }
    lines
}
