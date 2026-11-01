use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// The wizard buttons' ids from prsht.h; `PSM_SETNEXTTEXT` is ignored by this
/// sheet, but setting the controls' text directly works.
const ID_WIZBACK: u16 = 0x3023;
pub const ID_WIZNEXT: u16 = 0x3024;
pub const ID_WIZFINISH: u16 = 0x3025;

/// The sheet owning a page, which handles the `PSM_*` messages.
pub unsafe fn sheet(dialog: HWND) -> HWND {
    wm::GetParent(dialog)
}

/// Enables exactly these wizard buttons.
pub unsafe fn set_buttons(dialog: HWND, flags: u32) {
    wm::SendMessageW(sheet(dialog), ctl::PSM_SETWIZBUTTONS, 0, flags as LPARAM);
}

/// Labels the primary button for this scene, on both Next and Finish since
/// the sheet swaps which one shows, and translates Back and Cancel, which
/// comctl32 would otherwise leave in the OS language.
pub unsafe fn set_next_text(dialog: HWND, string_id: u16) {
    let sheet = sheet(dialog);
    let set = |control: u16, string_id: u16| {
        let text = crate::i18n::to_wide(&crate::i18n::load_string(string_id));
        wm::SetDlgItemTextW(sheet, control as i32, text.as_ptr());
    };
    set(ID_WIZNEXT, string_id);
    set(ID_WIZFINISH, string_id);
    set(ID_WIZBACK, crate::i18n::IDS_BTN_BACK);
    set(crate::ui::IDCANCEL, crate::i18n::IDS_BTN_CANCEL);
}

/// Set on activation because this sheet ignores `PROPSHEETHEADERW::pszCaption`.
pub unsafe fn set_sheet_caption(dialog: HWND) {
    let caption = crate::i18n::load_string(crate::i18n::IDS_APP_TITLE);
    crate::ui::set_window_text(sheet(dialog), &caption);
}

/// The scene title is a static in each template, because the Wizard97 header
/// shows a title only beside a header bitmap that would cost size budget.
pub unsafe fn set_header(dialog: HWND, string_id: u16) {
    crate::ui::set_text(
        dialog,
        crate::wizard::control_ids::IDC_SCENE_TITLE,
        string_id,
    );
}
