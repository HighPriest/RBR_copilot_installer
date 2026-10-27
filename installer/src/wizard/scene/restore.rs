use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// The restore scene: pick a state to go back to, read what that would do, then
/// do it. Mostly preview, since this is the one route that can remove files a
/// user has, and what exactly is about to happen is the question to answer first.
pub unsafe extern "system" fn restore_page(
    dialog: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match message {
        wm::WM_INITDIALOG => {
            crate::wizard::page::adopt_state(dialog, lparam);
            crate::ui::set_text(
                dialog,
                crate::wizard::control_ids::IDC_RESTORE_PROMPT,
                crate::i18n::IDS_RESTORE_PROMPT,
            );
            1
        }
        wm::WM_COMMAND => {
            if crate::ui::hiword(wparam) == crate::ui::LBN_SELCHANGE
                && crate::ui::loword(wparam) == crate::wizard::control_ids::IDC_RESTORE_POINTS
            {
                on_point_picked(dialog);
            }
            1
        }
        wm::WM_NOTIFY => match crate::wizard::page::notify_code(lparam) {
            Some(ctl::PSN_SETACTIVE) => {
                crate::wizard::page::set_sheet_caption(dialog);
                crate::wizard::page::set_header(dialog, crate::i18n::IDS_RESTORE_TITLE);
                crate::wizard::page::set_buttons(dialog, ctl::PSWIZB_BACK | ctl::PSWIZB_NEXT);
                crate::wizard::page::set_next_text(dialog, crate::i18n::IDS_BTN_RESTORE);
                fill_points(dialog);
                crate::wizard::page::reply(dialog, 0)
            }
            // Back to the folder page this route came from.
            Some(ctl::PSN_WIZBACK) => crate::wizard::page::reply(
                dialog,
                crate::wizard::control_ids::IDD_S3_FOLDER as isize,
            ),
            Some(ctl::PSN_WIZNEXT) => on_restore(dialog),
            Some(ctl::PSN_QUERYCANCEL) => crate::wizard::page::query_cancel(dialog),
            _ => 0,
        },
        _ => 0,
    }
}

/// Lists the points newest first, with "remove everything" last; rebuilt on
/// every activation, since a run since the last visit added one.
unsafe fn fill_points(dialog: HWND) {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return;
    };
    let points = crate::wizard::control_ids::IDC_RESTORE_POINTS;
    let preview = crate::wizard::control_ids::IDC_RESTORE_PREVIEW;
    let game_dir = std::path::PathBuf::from(&state.folder);
    state.points = pacenotes_installer::restore::points(&game_dir);
    state.restore_to = None;
    state.restore_plan = pacenotes_installer::restore::Plan::default();

    crate::ui::list_reset(dialog, points);
    crate::ui::set_text_raw(dialog, preview, "");
    crate::ui::enable(dialog, points, !state.points.is_empty());
    if state.points.is_empty() {
        crate::ui::set_text(dialog, preview, crate::i18n::IDS_RESTORE_EMPTY);
        return;
    }
    // The newest point is compared with the folder as it is now.
    let live = pacenotes_installer::ledger::Ledger::open(&game_dir)
        .packages()
        .clone();
    for (index, point) in state.points.iter().enumerate() {
        let after = match index {
            0 => &live,
            _ => &state.points[index - 1].snapshot,
        };
        crate::ui::list_add(dialog, points, &point_label(point, after));
    }
    let everything = crate::i18n::load_string(crate::i18n::IDS_RESTORE_ALL);
    crate::ui::list_add(dialog, points, &everything);
}

/// The date, plus what the run after the point did, which is what tells two dates apart.
fn point_label(
    point: &pacenotes_installer::restore::Point,
    after: &pacenotes_installer::restore::Snapshot,
) -> String {
    let changes = pacenotes_installer::restore::changes(&point.snapshot, after);
    let when = pacenotes_installer::restore::human_time(&point.name);
    if changes.is_empty() {
        return when;
    }
    format!(
        "{when}      +{} -{} ~{}",
        changes.added, changes.removed, changes.updated
    )
}

/// Works out the plan for the chosen row and shows it; the last row is a
/// restore to an empty state, the same operation rather than a second path.
unsafe fn on_point_picked(dialog: HWND) {
    let control = crate::wizard::control_ids::IDC_RESTORE_POINTS;
    let Some(row) = crate::ui::list_selection(dialog, control) else {
        return;
    };
    let Some(state) = crate::wizard::page::state(dialog) else {
        return;
    };
    let point =
        state
            .points
            .get(row)
            .cloned()
            .unwrap_or_else(|| pacenotes_installer::restore::Point {
                name: crate::i18n::load_string(crate::i18n::IDS_RESTORE_ALL),
                snapshot: pacenotes_installer::restore::Snapshot::new(),
            });
    let ledger = pacenotes_installer::ledger::Ledger::open(&state.folder);
    let cache = &state.cache;
    state.restore_plan =
        pacenotes_installer::restore::plan(&point.snapshot, ledger.packages(), &|sha256| {
            cache.has_payload(sha256)
        });
    state.restore_to = Some(point);
    let text = describe_restore(&state.restore_plan);
    crate::ui::set_text_raw(
        dialog,
        crate::wizard::control_ids::IDC_RESTORE_PREVIEW,
        &text,
    );
}

/// The preview, removals first because they are what a user must read before
/// agreeing; packages are named by their leaf, as every other scene names them.
fn describe_restore(plan: &pacenotes_installer::restore::Plan) -> String {
    if plan.is_empty() && plan.unavailable.is_empty() {
        return crate::i18n::load_string(crate::i18n::IDS_RESTORE_NOTHING);
    }
    let put_back = |wanted: &pacenotes_installer::restore::PutBack| wanted.key.clone();
    let sections = [
        (crate::i18n::IDS_RESTORE_REMOVE, plan.remove.clone()),
        (
            crate::i18n::IDS_RESTORE_PUT_BACK,
            plan.put_back.iter().map(put_back).collect(),
        ),
        (crate::i18n::IDS_RESTORE_KEEP, plan.keep.clone()),
        (
            crate::i18n::IDS_RESTORE_MISSING,
            plan.unavailable.iter().map(put_back).collect(),
        ),
    ];
    sections
        .iter()
        .filter(|(_, keys)| !keys.is_empty())
        .map(|(heading, keys)| crate::i18n::load_string_arg(*heading, &leaves(keys)))
        .collect::<Vec<_>>()
        .join("\r\n\r\n")
}

/// One indented line per package.
fn leaves(keys: &[String]) -> String {
    keys.iter()
        .map(|key| format!("    {}", key.rsplit('/').next().unwrap_or(key)))
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// Hands over to scene 6, which runs the restore as it runs an install; by id,
/// since this page is last in the sheet.
unsafe fn on_restore(dialog: HWND) -> isize {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return crate::wizard::page::reply(dialog, 0);
    };
    if state.restore_to.is_none() {
        let pick = crate::i18n::load_string(crate::i18n::IDS_RESTORE_PICK);
        crate::ui::message_box_warning(dialog, &pick);
        return crate::wizard::page::stay(dialog);
    }
    // A progress page that does nothing would be theatre.
    if state.restore_plan.is_empty() {
        let nothing = crate::i18n::load_string(crate::i18n::IDS_RESTORE_NOTHING);
        crate::ui::message_box_info(dialog, &nothing);
        return crate::wizard::page::stay(dialog);
    }
    crate::wizard::page::reply(dialog, crate::wizard::control_ids::IDD_S6_INSTALL as isize)
}
