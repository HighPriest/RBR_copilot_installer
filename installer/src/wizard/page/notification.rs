use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::Controls as ctl;

/// The first slot of a dialog's extra window memory; windows-sys does not export it.
const DWLP_MSGRESULT: i32 = 0;

/// The code of an incoming `WM_NOTIFY`.
pub unsafe fn notify_code(lparam: LPARAM) -> Option<u32> {
    let header = lparam as *const ctl::NMHDR;
    header.as_ref().map(|header| header.code)
}

/// Answers a `PSN_*` notification with `result`.
pub unsafe fn reply(dialog: HWND, result: isize) -> isize {
    windows_sys::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(dialog, DWLP_MSGRESULT, result);
    1
}

/// Refuses to leave the page.
pub unsafe fn stay(dialog: HWND) -> isize {
    reply(dialog, ctl::PSNRET_INVALID_NOCHANGEPAGE as isize)
}

/// Cancel asks first; a nonzero reply vetoes the close.
pub unsafe fn query_cancel(dialog: HWND) -> isize {
    let confirmed = crate::ui::confirm(
        dialog,
        &crate::i18n::load_string(crate::i18n::IDS_CANCEL_CONFIRM),
    );
    reply(dialog, isize::from(!confirmed))
}
