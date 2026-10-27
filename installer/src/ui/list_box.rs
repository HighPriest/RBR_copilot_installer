const LB_ADDSTRING: u32 = 0x0180;
const LB_RESETCONTENT: u32 = 0x0184;
const LB_SETCURSEL: u32 = 0x0186;
const LB_GETCURSEL: u32 = 0x0188;
/// `LB_SETCURSEL`'s index for "nothing selected".
const NO_SELECTION: isize = -1;

/// The notification a list box sends its parent when the user changes the
/// selection; windows-sys does not export it.
pub const LBN_SELCHANGE: u16 = 1;

pub unsafe fn list_reset(dialog: windows_sys::Win32::Foundation::HWND, control: u16) {
    windows_sys::Win32::UI::WindowsAndMessaging::SendDlgItemMessageW(
        dialog,
        control as i32,
        LB_RESETCONTENT,
        0,
        0,
    );
}

pub unsafe fn list_add(dialog: windows_sys::Win32::Foundation::HWND, control: u16, text: &str) {
    let wide = crate::i18n::to_wide(text);
    windows_sys::Win32::UI::WindowsAndMessaging::SendDlgItemMessageW(
        dialog,
        control as i32,
        LB_ADDSTRING,
        0,
        wide.as_ptr() as windows_sys::Win32::Foundation::LPARAM,
    );
}

/// Selects an item, or clears the selection for `None`.
pub unsafe fn list_select(
    dialog: windows_sys::Win32::Foundation::HWND,
    control: u16,
    index: Option<usize>,
) {
    let item = index.map_or(NO_SELECTION, |index| index as isize);
    windows_sys::Win32::UI::WindowsAndMessaging::SendDlgItemMessageW(
        dialog,
        control as i32,
        LB_SETCURSEL,
        item as windows_sys::Win32::Foundation::WPARAM,
        0,
    );
}

/// `None` when nothing is selected.
pub unsafe fn list_selection(
    dialog: windows_sys::Win32::Foundation::HWND,
    control: u16,
) -> Option<usize> {
    let selection = windows_sys::Win32::UI::WindowsAndMessaging::SendDlgItemMessageW(
        dialog,
        control as i32,
        LB_GETCURSEL,
        0,
        0,
    );
    usize::try_from(selection).ok()
}
