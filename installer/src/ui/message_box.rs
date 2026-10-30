const IDYES: i32 = 6;

/// A failure the user must know about.
pub fn message_box_error(owner: windows_sys::Win32::Foundation::HWND, text: &str) {
    show(
        owner,
        text,
        windows_sys::Win32::UI::WindowsAndMessaging::MB_OK
            | windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
    );
}

/// Something to correct before the wizard can move on.
pub fn message_box_warning(owner: windows_sys::Win32::Foundation::HWND, text: &str) {
    show(
        owner,
        text,
        windows_sys::Win32::UI::WindowsAndMessaging::MB_OK
            | windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONWARNING,
    );
}

pub fn message_box_info(owner: windows_sys::Win32::Foundation::HWND, text: &str) {
    show(
        owner,
        text,
        windows_sys::Win32::UI::WindowsAndMessaging::MB_OK
            | windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONINFORMATION,
    );
}

/// Yes/No; true on Yes.
pub fn confirm(owner: windows_sys::Win32::Foundation::HWND, text: &str) -> bool {
    show(
        owner,
        text,
        windows_sys::Win32::UI::WindowsAndMessaging::MB_YESNO
            | windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONQUESTION,
    ) == IDYES
}

/// Every box carries the localized application title.
fn show(owner: windows_sys::Win32::Foundation::HWND, text: &str, style: u32) -> i32 {
    let body = crate::i18n::to_wide(text);
    let title = crate::i18n::to_wide(&crate::i18n::load_string(crate::i18n::IDS_APP_TITLE));
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
            owner,
            body.as_ptr(),
            title.as_ptr(),
            style,
        )
    }
}
