//! Small Win32 helpers shared by the wizard pages.
//!
//! The scenes themselves live in `wizard.rs`; this is the vocabulary they are
//! written in — reading and writing control text, the three modal shapes, and
//! the folder picker. Only this module, `wizard.rs`, `http.rs`, `worker.rs`
//! and `main.rs` may call windows-sys (installer/CLAUDE.md).

#![allow(dead_code)]

use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

pub const IDOK: u16 = 1;
pub const IDCANCEL: u16 = 2;
pub const IDYES: i32 = 6;

/// MAKEINTRESOURCEW — a resource id masquerading as a wide string pointer.
pub fn make_int_resource(id: u16) -> windows_sys::core::PCWSTR {
    id as usize as windows_sys::core::PCWSTR
}

pub fn loword(w: WPARAM) -> u16 {
    (w & 0xFFFF) as u16
}

pub fn hiword(w: WPARAM) -> u16 {
    ((w >> 16) & 0xFFFF) as u16
}

// ---- Control text ----

/// Set a control's caption from the string table.
pub unsafe fn set_text(dlg: HWND, id: u16, string_id: u16) {
    let w = crate::i18n::to_wide(&crate::i18n::load_string(string_id));
    wm::SetDlgItemTextW(dlg, id as i32, w.as_ptr());
}

pub unsafe fn set_text_raw(dlg: HWND, id: u16, text: &str) {
    let w = crate::i18n::to_wide(text);
    wm::SetDlgItemTextW(dlg, id as i32, w.as_ptr());
}

/// Read a control's text.
///
/// GetDlgItemTextW rather than GetWindowText: within our own process either
/// works, but this one is bounded and needs no second call to size a buffer.
pub unsafe fn get_text(dlg: HWND, id: u16) -> String {
    let mut buf = [0u16; 1024];
    let len = wm::GetDlgItemTextW(dlg, id as i32, buf.as_mut_ptr(), buf.len() as i32);
    String::from_utf16_lossy(&buf[..len as usize])
}

pub unsafe fn set_window_text(hwnd: HWND, text: &str) {
    let w = crate::i18n::to_wide(text);
    wm::SetWindowTextW(hwnd, w.as_ptr());
}

pub unsafe fn enable(dlg: HWND, id: u16, on: bool) {
    windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
        wm::GetDlgItem(dlg, id as i32),
        i32::from(on),
    );
}

// ---- Combo boxes ----

pub const CB_ADDSTRING: u32 = 0x0143;
pub const CB_SETCURSEL: u32 = 0x014E;
pub const CB_GETCURSEL: u32 = 0x0147;
pub const CB_RESETCONTENT: u32 = 0x014B;

pub unsafe fn combo_reset(dlg: HWND, id: u16) {
    wm::SendDlgItemMessageW(dlg, id as i32, CB_RESETCONTENT, 0, 0);
}

pub unsafe fn combo_add(dlg: HWND, id: u16, text: &str) {
    let w = crate::i18n::to_wide(text);
    wm::SendDlgItemMessageW(dlg, id as i32, CB_ADDSTRING, 0, w.as_ptr() as LPARAM);
}

pub unsafe fn combo_select(dlg: HWND, id: u16, index: usize) {
    wm::SendDlgItemMessageW(dlg, id as i32, CB_SETCURSEL, index, 0);
}

/// Selected index, or None when nothing is selected (CB_ERR).
pub unsafe fn combo_selection(dlg: HWND, id: u16) -> Option<usize> {
    let sel = wm::SendDlgItemMessageW(dlg, id as i32, CB_GETCURSEL, 0, 0);
    if sel < 0 {
        None
    } else {
        Some(sel as usize)
    }
}

// ---- Modals ----

/// Localized title for every box we raise.
unsafe fn app_title() -> Vec<u16> {
    crate::i18n::to_wide(&crate::i18n::load_string(crate::i18n::IDS_APP_TITLE))
}

fn message_box(owner: HWND, text: &str, icon: u32) -> i32 {
    let body = crate::i18n::to_wide(text);
    unsafe {
        let title = app_title();
        wm::MessageBoxW(owner, body.as_ptr(), title.as_ptr(), wm::MB_OK | icon)
    }
}

/// Something failed and the user must know (CLAUDE.md: no silent failures).
pub fn message_box_error(owner: HWND, text: &str) {
    message_box(owner, text, wm::MB_ICONERROR);
}

/// The user asked for something we cannot do yet, or did something that needs
/// correcting before the wizard can move on. DESIGN.md calls these "warning
/// modals" on scenes 2 and 3.
pub fn message_box_warning(owner: HWND, text: &str) {
    message_box(owner, text, wm::MB_ICONWARNING);
}

pub fn message_box_info(owner: HWND, text: &str) {
    message_box(owner, text, wm::MB_ICONINFORMATION);
}

/// Yes/No confirmation; true on Yes. Used for Cancel (DESIGN.md: Cancel asks).
pub fn confirm(owner: HWND, text: &str) -> bool {
    let body = crate::i18n::to_wide(text);
    unsafe {
        let title = app_title();
        wm::MessageBoxW(
            owner,
            body.as_ptr(),
            title.as_ptr(),
            wm::MB_YESNO | wm::MB_ICONQUESTION,
        ) == IDYES
    }
}

// ---- Folder picker ----

const BIF_RETURNONLYFSDIRS: u32 = 0x0000_0001;
const BIF_NEWDIALOGSTYLE: u32 = 0x0000_0040;

/// SHBrowseForFolderW → picked path. None on cancel.
pub unsafe fn browse_for_folder(owner: HWND, prompt: &str) -> Option<String> {
    let title = crate::i18n::to_wide(prompt);
    let mut bi: windows_sys::Win32::UI::Shell::BROWSEINFOW = core::mem::zeroed();
    bi.hwndOwner = owner;
    bi.lpszTitle = title.as_ptr();
    bi.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;

    let pidl = windows_sys::Win32::UI::Shell::SHBrowseForFolderW(&bi);
    if pidl.is_null() {
        return None;
    }
    let mut buf = [0u16; 260]; // MAX_PATH
    let ok = windows_sys::Win32::UI::Shell::SHGetPathFromIDListW(pidl, buf.as_mut_ptr());
    windows_sys::Win32::System::Com::CoTaskMemFree(pidl as *const core::ffi::c_void);
    if ok == 0 {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

// ---- Icons ----

pub unsafe fn load_app_icon() -> windows_sys::Win32::UI::WindowsAndMessaging::HICON {
    wm::LoadIconW(crate::i18n::instance(), make_int_resource(1))
}
