use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

const LBN_SELCHANGE: u16 = 1;

/// `(list, label)` per list, in `CHOICE_CATEGORIES` order, so index i is the model's list i.
const LISTS: [(u16, u16); 3] = [
    (
        crate::wizard::control_ids::IDC_S5_LANG,
        crate::wizard::control_ids::IDC_S5_LANG_LABEL,
    ),
    (
        crate::wizard::control_ids::IDC_S5_VOICE,
        crate::wizard::control_ids::IDC_S5_VOICE_LABEL,
    ),
    (
        crate::wizard::control_ids::IDC_S5_STYLE,
        crate::wizard::control_ids::IDC_S5_STYLE_LABEL,
    ),
];

/// Scene 5: one pick each of language, co-driver and style.
pub unsafe extern "system" fn configuration_page(
    dialog: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match message {
        wm::WM_INITDIALOG => {
            crate::wizard::page::adopt_state(dialog, lparam);
            for ((_, label), category) in LISTS.iter().zip(pacenotes_shared::CHOICE_CATEGORIES) {
                crate::ui::set_text(dialog, *label, category_caption(category));
            }
            1
        }
        wm::WM_COMMAND => {
            if crate::ui::hiword(wparam) == LBN_SELCHANGE {
                on_choice_picked(dialog, crate::ui::loword(wparam));
            }
            1
        }
        wm::WM_NOTIFY => match crate::wizard::page::notify_code(lparam) {
            Some(ctl::PSN_SETACTIVE) => {
                crate::wizard::page::set_sheet_caption(dialog);
                crate::wizard::page::set_header(dialog, crate::i18n::IDS_S5_TITLE);
                crate::wizard::page::set_buttons(dialog, ctl::PSWIZB_BACK | ctl::PSWIZB_NEXT);
                crate::wizard::page::set_next_text(dialog, crate::i18n::IDS_BTN_INSTALL);
                populate_lists(dialog);
                crate::wizard::page::reply(dialog, 0)
            }
            Some(ctl::PSN_WIZNEXT) => on_install(dialog),
            Some(ctl::PSN_QUERYCANCEL) => crate::wizard::page::query_cancel(dialog),
            _ => 0,
        },
        _ => 0,
    }
}

/// Rebuilt on every activation, since the user may have gone back and changed the ticks.
unsafe fn populate_lists(dialog: HWND) {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return;
    };
    let keep = state.config.selected_keys();
    state.config = pacenotes_installer::choices::Configuration::from_tree(&state.tree, &keep);

    let nothing_ticked = crate::i18n::load_string(crate::i18n::IDS_S5_NONE);
    for (&(list_control, _), list) in LISTS.iter().zip(&state.config.lists) {
        crate::ui::list_reset(dialog, list_control);
        if list.options.is_empty() {
            // Disabled, so the explanation cannot be mistaken for an option.
            crate::ui::list_add(dialog, list_control, &nothing_ticked);
            crate::ui::enable(dialog, list_control, false);
            continue;
        }
        crate::ui::enable(dialog, list_control, true);
        for option in &list.options {
            crate::ui::list_add(dialog, list_control, &option.label);
        }
        crate::ui::list_select(dialog, list_control, list.selected);
    }
}

unsafe fn on_choice_picked(dialog: HWND, control: u16) {
    let Some(index) = LISTS.iter().position(|&(list, _)| list == control) else {
        return;
    };
    let selection = crate::ui::list_selection(dialog, control);
    if let Some(state) = crate::wizard::page::state(dialog) {
        state.config.select(index, selection);
    }
}

/// Every list that offers anything needs a pick; the warning names them by their captions.
unsafe fn on_install(dialog: HWND) -> isize {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return crate::wizard::page::reply(dialog, 0);
    };
    let missing = state.config.missing();
    if missing.is_empty() {
        return crate::wizard::page::reply(dialog, 0);
    }
    let named: Vec<String> = missing
        .iter()
        .map(|category| {
            crate::i18n::load_string(category_caption(category))
                .trim_end_matches(':')
                .to_string()
        })
        .collect();
    let text = crate::i18n::load_string_arg(crate::i18n::IDS_S5_MISSING, &named.join(", "));
    crate::ui::message_box_warning(dialog, &text);
    crate::wizard::page::stay(dialog)
}

/// Matched by category rather than position, so a reordered list cannot be mislabelled.
fn category_caption(category: &str) -> u16 {
    match category {
        pacenotes_shared::CATEGORY_LANGUAGE => crate::i18n::IDS_S5_LANGUAGE,
        pacenotes_shared::CATEGORY_CODRIVER => crate::i18n::IDS_S5_VOICE,
        pacenotes_shared::CATEGORY_STYLE => crate::i18n::IDS_S5_STYLE,
        _ => crate::i18n::IDS_S5_TITLE,
    }
}
