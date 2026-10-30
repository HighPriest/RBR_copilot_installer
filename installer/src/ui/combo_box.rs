const CB_ADDSTRING: u32 = 0x0143;
const CB_SETCURSEL: u32 = 0x014E;
const CB_GETCURSEL: u32 = 0x0147;
const CB_RESETCONTENT: u32 = 0x014B;

pub unsafe fn combo_reset(dialog: windows_sys::Win32::Foundation::HWND, control: u16) {
    windows_sys::Win32::UI::WindowsAndMessaging::SendDlgItemMessageW(
        dialog,
        control as i32,
        CB_RESETCONTENT,
        0,
        0,
    );
}

pub unsafe fn combo_add(dialog: windows_sys::Win32::Foundation::HWND, control: u16, text: &str) {
    let wide = crate::i18n::to_wide(text);
    windows_sys::Win32::UI::WindowsAndMessaging::SendDlgItemMessageW(
        dialog,
        control as i32,
        CB_ADDSTRING,
        0,
        wide.as_ptr() as windows_sys::Win32::Foundation::LPARAM,
    );
}

pub unsafe fn combo_select(
    dialog: windows_sys::Win32::Foundation::HWND,
    control: u16,
    index: usize,
) {
    windows_sys::Win32::UI::WindowsAndMessaging::SendDlgItemMessageW(
        dialog,
        control as i32,
        CB_SETCURSEL,
        index,
        0,
    );
}

/// `None` when nothing is selected.
pub unsafe fn combo_selection(
    dialog: windows_sys::Win32::Foundation::HWND,
    control: u16,
) -> Option<usize> {
    let selection = windows_sys::Win32::UI::WindowsAndMessaging::SendDlgItemMessageW(
        dialog,
        control as i32,
        CB_GETCURSEL,
        0,
        0,
    );
    usize::try_from(selection).ok()
}
