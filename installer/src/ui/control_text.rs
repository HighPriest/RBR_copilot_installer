/// Sets a dialog control's caption from the string table.
pub unsafe fn set_text(dialog: windows_sys::Win32::Foundation::HWND, control: u16, string_id: u16) {
    set_text_raw(dialog, control, &crate::i18n::load_string(string_id));
}

pub unsafe fn set_text_raw(dialog: windows_sys::Win32::Foundation::HWND, control: u16, text: &str) {
    let wide = crate::i18n::to_wide(text);
    windows_sys::Win32::UI::WindowsAndMessaging::SetDlgItemTextW(
        dialog,
        control as i32,
        wide.as_ptr(),
    );
}

/// Adds a line at the end of a multi-line edit control, which scrolls to it.
/// Setting the whole text again for each line made a long run's log
/// quadratic: 20 s for 1 500 lines, measured on the control itself.
pub unsafe fn append_line(dialog: windows_sys::Win32::Foundation::HWND, control: u16, line: &str) {
    let edit = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(dialog, control as i32);
    if edit.is_null() {
        return;
    }
    let length = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextLengthW(edit) as usize;
    let text = if length == 0 {
        crate::i18n::to_wide(line)
    } else {
        crate::i18n::to_wide(&format!("\r\n{line}"))
    };
    let send = windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;
    // A pane appended to is capped at 30 000 characters unless lifted; 0 lifts it.
    send(
        edit,
        windows_sys::Win32::UI::Controls::EM_SETLIMITTEXT,
        0,
        0,
    );
    send(
        edit,
        windows_sys::Win32::UI::Controls::EM_SETSEL,
        length,
        length as isize,
    );
    send(
        edit,
        windows_sys::Win32::UI::Controls::EM_REPLACESEL,
        0,
        text.as_ptr() as isize,
    );
}

/// At most 1023 characters, which no control here holds more of.
pub unsafe fn get_text(dialog: windows_sys::Win32::Foundation::HWND, control: u16) -> String {
    let mut buffer = [0u16; 1024];
    let length = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItemTextW(
        dialog,
        control as i32,
        buffer.as_mut_ptr(),
        buffer.len() as i32,
    );
    String::from_utf16_lossy(&buffer[..length as usize])
}

pub unsafe fn set_window_text(window: windows_sys::Win32::Foundation::HWND, text: &str) {
    let wide = crate::i18n::to_wide(text);
    windows_sys::Win32::UI::WindowsAndMessaging::SetWindowTextW(window, wide.as_ptr());
}

pub unsafe fn enable(dialog: windows_sys::Win32::Foundation::HWND, control: u16, enabled: bool) {
    windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
        windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(dialog, control as i32),
        i32::from(enabled),
    );
}
