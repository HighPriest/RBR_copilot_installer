use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// Scene 1: pick the language every later page is built in.
pub unsafe extern "system" fn language_page(
    dialog: HWND,
    message: u32,
    _wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match message {
        wm::WM_INITDIALOG => {
            initialize(dialog, lparam);
            1
        }
        wm::WM_NOTIFY => match crate::wizard::page::notify_code(lparam) {
            Some(ctl::PSN_SETACTIVE) => {
                crate::wizard::page::set_sheet_caption(dialog);
                crate::wizard::page::set_header(dialog, crate::i18n::IDS_S1_TITLE);
                crate::wizard::page::set_buttons(dialog, ctl::PSWIZB_NEXT);
                crate::wizard::page::set_next_text(dialog, crate::i18n::IDS_BTN_CONTINUE);
                crate::wizard::page::reply(dialog, 0)
            }
            Some(ctl::PSN_WIZNEXT) => {
                apply_pick(dialog);
                crate::wizard::page::reply(dialog, 0)
            }
            Some(ctl::PSN_QUERYCANCEL) => crate::wizard::page::query_cancel(dialog),
            _ => 0,
        },
        _ => 0,
    }
}

unsafe fn initialize(dialog: HWND, lparam: LPARAM) {
    let state = crate::wizard::page::adopt_state(dialog, lparam);
    let combo = crate::wizard::control_ids::IDC_S1_COMBO;
    crate::ui::set_text(
        dialog,
        crate::wizard::control_ids::IDC_S1_PROMPT,
        crate::i18n::IDS_S1_PROMPT,
    );
    crate::ui::set_text(
        dialog,
        crate::wizard::control_ids::IDC_S1_NOTE,
        crate::i18n::IDS_S1_RESTART,
    );
    for language in crate::i18n::Language::ALL {
        crate::ui::combo_add(dialog, combo, language.label());
    }
    let initial = crate::i18n::os_language().unwrap_or(crate::i18n::Language::En);
    crate::ui::combo_select(dialog, combo, initial.index());
    if let Some(state) = state {
        state.language = initial;
    }
}

/// Later pages are created after this, so they come up in the picked language.
unsafe fn apply_pick(dialog: HWND) {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return;
    };
    let picked = crate::ui::combo_selection(dialog, crate::wizard::control_ids::IDC_S1_COMBO)
        .map(crate::i18n::Language::from_index)
        .unwrap_or(crate::i18n::Language::En);
    state.language = picked;
    crate::i18n::set_language(picked);
}
