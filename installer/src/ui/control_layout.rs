const EM_SETSEL: u32 = 0x00B1;
const EM_SCROLLCARET: u32 = 0x00B7;

/// Scrolls a multiline edit to its last line, so a growing log never looks stalled.
pub unsafe fn scroll_to_end(dialog: windows_sys::Win32::Foundation::HWND, control: u16) {
    let edit = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(dialog, control as i32);
    let end = windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(
        edit,
        windows_sys::Win32::UI::WindowsAndMessaging::WM_GETTEXTLENGTH,
        0,
        0,
    );
    // The caret has to sit at the end; `EM_SETSEL(-1, -1)` only clears the selection.
    windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(
        edit,
        EM_SETSEL,
        end as windows_sys::Win32::Foundation::WPARAM,
        end,
    );
    windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(edit, EM_SCROLLCARET, 0, 0);
}

pub unsafe fn show(dialog: windows_sys::Win32::Foundation::HWND, control: u16, visible: bool) {
    let mode = if visible {
        windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW
    } else {
        windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE
    };
    windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
        windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(dialog, control as i32),
        mode,
    );
}

/// Grows `control` down to the bottom edge of `over`, which takes the room of a hidden control.
pub unsafe fn stretch_to_bottom_of(
    dialog: windows_sys::Win32::Foundation::HWND,
    control: u16,
    over: u16,
) {
    let (Some(grown), Some(covered)) = (
        rect_in_dialog(dialog, control),
        rect_in_dialog(dialog, over),
    ) else {
        return;
    };
    windows_sys::Win32::UI::WindowsAndMessaging::SetWindowPos(
        windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(dialog, control as i32),
        core::ptr::null_mut(),
        0,
        0,
        grown.right - grown.left,
        covered.bottom - grown.top,
        windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
            | windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOZORDER
            | windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE,
    );
}

/// A control's rectangle in its dialog's client coordinates, which `SetWindowPos` takes.
unsafe fn rect_in_dialog(
    dialog: windows_sys::Win32::Foundation::HWND,
    control: u16,
) -> Option<windows_sys::Win32::Foundation::RECT> {
    let mut rect: windows_sys::Win32::Foundation::RECT = core::mem::zeroed();
    let window = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(dialog, control as i32);
    if windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(window, &mut rect) == 0 {
        return None;
    }
    let corner_count = 2;
    windows_sys::Win32::Graphics::Gdi::MapWindowPoints(
        core::ptr::null_mut(),
        dialog,
        &mut rect as *mut _ as *mut windows_sys::Win32::Foundation::POINT,
        corner_count,
    );
    Some(rect)
}
