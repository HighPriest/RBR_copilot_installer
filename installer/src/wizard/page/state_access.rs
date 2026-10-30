use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// The shared state stored in the page's `GWLP_USERDATA` by `adopt_state`.
pub unsafe fn state<'a>(dialog: HWND) -> Option<&'a mut crate::wizard::state::WizardState> {
    (wm::GetWindowLongPtrW(dialog, wm::GWLP_USERDATA) as *mut crate::wizard::state::WizardState)
        .as_mut()
}

/// Stores the state that `WM_INITDIALOG` delivers inside the page's `PROPSHEETPAGEW`.
pub unsafe fn adopt_state<'a>(
    dialog: HWND,
    lparam: LPARAM,
) -> Option<&'a mut crate::wizard::state::WizardState> {
    let page = lparam as *const windows_sys::Win32::UI::Controls::PROPSHEETPAGEW;
    let pointer = if page.is_null() { 0 } else { (*page).lParam };
    wm::SetWindowLongPtrW(dialog, wm::GWLP_USERDATA, pointer);
    (pointer as *mut crate::wizard::state::WizardState).as_mut()
}
