/// Shows `path` in Explorer with it selected, since what the user needs is to
/// find the file, usually to attach it, and a `.log` may open with anything.
pub fn reveal_in_explorer(path: &std::path::Path) {
    let verb = crate::i18n::to_wide("open");
    let target = crate::i18n::to_wide("explorer.exe");
    let arguments = crate::i18n::to_wide(&format!("/select,\"{}\"", path.display()));
    unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            core::ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            arguments.as_ptr(),
            core::ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
}
