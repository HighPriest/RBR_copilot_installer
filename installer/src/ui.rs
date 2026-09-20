//! Win32 dialog procs and UI plumbing. Control IDs MUST match res/installer.rc.
//! Only this module + http.rs + main.rs may call windows-sys (see CLAUDE.md).
//! Task I2.

#![allow(dead_code)]

use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

// ---- Control IDs (mirror res/installer.rc) ----
pub const IDD_LANGPICK: u16 = 100;
pub const IDC_LANGCOMBO: u16 = 101;
pub const IDC_LANGPROMPT: u16 = 102;

pub const IDD_MAIN: u16 = 110;
pub const IDC_TAB: u16 = 111;

pub const IDC_LIB_LIST: u16 = 120;
pub const IDC_LIB_TOKENLABEL: u16 = 121;
pub const IDC_LIB_TOKEN: u16 = 122;
pub const IDC_LIB_UNLOCK: u16 = 123;
pub const IDC_LIB_DOWNLOAD: u16 = 124;

pub const IDC_INS_DIRLABEL: u16 = 140;
pub const IDC_INS_GAMEDIR: u16 = 141;
pub const IDC_INS_BROWSE: u16 = 142;
pub const IDC_INS_LIST: u16 = 143;
pub const IDC_INS_APPLY: u16 = 144;
pub const IDC_INS_RESTORE: u16 = 145;

pub const IDC_PROGRESS: u16 = 160;
pub const IDC_STATUS: u16 = 161;

/// Controls belonging to each view, flipped wholesale on tab change.
const LIBRARY_VIEW: [u16; 5] = [
    IDC_LIB_LIST,
    IDC_LIB_TOKENLABEL,
    IDC_LIB_TOKEN,
    IDC_LIB_UNLOCK,
    IDC_LIB_DOWNLOAD,
];
const INSTALL_VIEW: [u16; 6] = [
    IDC_INS_DIRLABEL,
    IDC_INS_GAMEDIR,
    IDC_INS_BROWSE,
    IDC_INS_LIST,
    IDC_INS_APPLY,
    IDC_INS_RESTORE,
];

const TAB_LIBRARY: i32 = 0;
const TAB_INSTALL: i32 = 1;

// ---- Worker → UI messages (task I4). WPARAM/LPARAM payloads documented. ----
pub const WM_APP: u32 = 0x8000;
/// wparam = percent (0..=100), lparam = string-table id for status line.
pub const WM_WORKER_PROGRESS: u32 = WM_APP + 1;
/// lparam = Box<String> (leaked, reclaimed by receiver) for dynamic status.
pub const WM_WORKER_STATUS: u32 = WM_APP + 2;
pub const WM_WORKER_DONE: u32 = WM_APP + 3;
/// lparam = Box<String> error message for MessageBox.
pub const WM_WORKER_ERROR: u32 = WM_APP + 4;

// ============================================================
// Language picker
// ============================================================

/// DialogBoxParamW(IDD_LANGPICK). Returns None on Cancel, which exits the app
/// (same as the C# LocalizationChangeForm driving Program.cs).
pub fn show_language_picker() -> Option<crate::i18n::Language> {
    let result = unsafe {
        wm::DialogBoxParamW(
            crate::i18n::instance(),
            make_int_resource(IDD_LANGPICK),
            core::ptr::null_mut(),
            Some(langpick_proc),
            0,
        )
    };
    // The proc passes back the selected index + 1 so that 0 stays "cancelled".
    match result {
        r if r > 0 => crate::i18n::Language::ALL.get((r - 1) as usize).copied(),
        _ => None,
    }
}

unsafe extern "system" fn langpick_proc(
    dlg: HWND,
    msg: u32,
    wparam: WPARAM,
    _lparam: LPARAM,
) -> isize {
    match msg {
        wm::WM_INITDIALOG => {
            set_dialog_icon(dlg);
            // The picker itself cannot be localized by the user's choice yet,
            // so it shows whatever the OS UI language resolves to.
            set_text(dlg, IDC_LANGPROMPT, crate::i18n::IDS_LANG_PROMPT);
            set_window_text(dlg, &crate::i18n::load_string(crate::i18n::IDS_LANG_TITLE));
            for lang in crate::i18n::Language::ALL {
                let w = crate::i18n::to_wide(lang.label());
                wm::SendDlgItemMessageW(
                    dlg,
                    IDC_LANGCOMBO as i32,
                    CB_ADDSTRING,
                    0,
                    w.as_ptr() as LPARAM,
                );
            }
            wm::SendDlgItemMessageW(dlg, IDC_LANGCOMBO as i32, CB_SETCURSEL, 0, 0);
            1
        }
        wm::WM_COMMAND => match loword(wparam) {
            IDOK => {
                let sel = wm::SendDlgItemMessageW(dlg, IDC_LANGCOMBO as i32, CB_GETCURSEL, 0, 0);
                // +1 so a valid index 0 is distinguishable from "cancelled".
                let picked = if sel < 0 { 0 } else { sel + 1 };
                wm::EndDialog(dlg, picked);
                1
            }
            IDCANCEL => {
                wm::EndDialog(dlg, 0);
                1
            }
            _ => 0,
        },
        _ => 0,
    }
}

// ============================================================
// Main dialog
// ============================================================

/// DialogBoxParamW(IDD_MAIN); pumps until the dialog closes.
pub fn run_main_dialog() {
    unsafe {
        wm::DialogBoxParamW(
            crate::i18n::instance(),
            make_int_resource(IDD_MAIN),
            core::ptr::null_mut(),
            Some(main_proc),
            0,
        );
    }
}

unsafe extern "system" fn main_proc(dlg: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    match msg {
        wm::WM_INITDIALOG => {
            init_main_dialog(dlg);
            1
        }
        wm::WM_NOTIFY => {
            let hdr = lparam as *const windows_sys::Win32::UI::Controls::NMHDR;
            if !hdr.is_null() && (*hdr).idFrom == IDC_TAB as usize && (*hdr).code == TCN_SELCHANGE {
                let sel =
                    wm::SendMessageW(wm::GetDlgItem(dlg, IDC_TAB as i32), TCM_GETCURSEL, 0, 0);
                show_view(dlg, sel as i32);
            }
            0
        }
        wm::WM_COMMAND => {
            on_command(dlg, loword(wparam));
            1
        }
        wm::WM_CLOSE => {
            wm::EndDialog(dlg, 0);
            1
        }
        _ => 0,
    }
}

unsafe fn init_main_dialog(dlg: HWND) {
    set_dialog_icon(dlg);
    set_window_text(dlg, &crate::i18n::load_string(crate::i18n::IDS_APP_TITLE));

    // The tab control and the view controls are siblings laid out on top of
    // each other. Controls earlier in the template sit higher in the Z-order,
    // so without this the tab would paint over both views.
    let tab = wm::GetDlgItem(dlg, IDC_TAB as i32);
    wm::SetWindowPos(
        tab,
        wm::HWND_BOTTOM,
        0,
        0,
        0,
        0,
        wm::SWP_NOMOVE | wm::SWP_NOSIZE | wm::SWP_NOACTIVATE,
    );

    add_tab(tab, TAB_LIBRARY, crate::i18n::IDS_TAB_LIBRARY);
    add_tab(tab, TAB_INSTALL, crate::i18n::IDS_TAB_INSTALL);

    // Localize every caption; the .rc text is only an English fallback.
    set_text(dlg, IDC_LIB_TOKENLABEL, crate::i18n::IDS_LBL_ACCESSKEY);
    set_text(dlg, IDC_LIB_UNLOCK, crate::i18n::IDS_BTN_UNLOCK);
    set_text(dlg, IDC_LIB_DOWNLOAD, crate::i18n::IDS_BTN_DOWNLOAD);
    set_text(dlg, IDC_INS_DIRLABEL, crate::i18n::IDS_LBL_GAMEDIR);
    set_text(dlg, IDC_INS_BROWSE, crate::i18n::IDS_BTN_BROWSE);
    set_text(dlg, IDC_INS_APPLY, crate::i18n::IDS_BTN_APPLY);
    set_text(dlg, IDC_INS_RESTORE, crate::i18n::IDS_BTN_RESTORE);
    set_text(dlg, IDCANCEL, crate::i18n::IDS_BTN_CLOSE);

    init_library_list(wm::GetDlgItem(dlg, IDC_LIB_LIST as i32));
    init_install_list(wm::GetDlgItem(dlg, IDC_INS_LIST as i32));

    status(dlg, &crate::i18n::load_string(crate::i18n::IDS_READY));
    show_view(dlg, TAB_LIBRARY);
}

unsafe fn init_library_list(list: HWND) {
    wm::SendMessageW(
        list,
        LVM_SETEXTENDEDLISTVIEWSTYLE,
        0,
        (LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES) as LPARAM,
    );
    add_column(list, 0, crate::i18n::IDS_COL_NAME, 140);
    add_column(list, 1, crate::i18n::IDS_COL_CATEGORY, 80);
    add_column(list, 2, crate::i18n::IDS_COL_VERSION, 60);
    add_column(list, 3, crate::i18n::IDS_COL_SIZE, 70);
    add_column(list, 4, crate::i18n::IDS_COL_STATE, 110);
}

unsafe fn init_install_list(list: HWND) {
    // Checkboxes drive the Apply selection; radio behavior inside an
    // exclusive_group is enforced in LVN_ITEMCHANGED once I5 supplies groups.
    wm::SendMessageW(
        list,
        LVM_SETEXTENDEDLISTVIEWSTYLE,
        0,
        (LVS_EX_FULLROWSELECT | LVS_EX_CHECKBOXES) as LPARAM,
    );
    add_column(list, 0, crate::i18n::IDS_COL_NAME, 180);
    add_column(list, 1, crate::i18n::IDS_COL_CATEGORY, 90);
    add_column(list, 2, crate::i18n::IDS_COL_VERSION, 70);
    add_column(list, 3, crate::i18n::IDS_COL_STATE, 110);
}

unsafe fn on_command(dlg: HWND, id: u16) {
    match id {
        IDCANCEL => {
            wm::EndDialog(dlg, 0);
        }
        IDC_INS_BROWSE => {
            if let Some(dir) = browse_for_folder(dlg) {
                let w = crate::i18n::to_wide(&dir);
                wm::SetDlgItemTextW(dlg, IDC_INS_GAMEDIR as i32, w.as_ptr());
            }
        }
        // Everything below needs the cache/engine work in I5-I7. Until then
        // say so in the status line rather than failing silently.
        IDC_LIB_DOWNLOAD | IDC_LIB_UNLOCK | IDC_INS_APPLY | IDC_INS_RESTORE => {
            status(dlg, &crate::i18n::load_string(crate::i18n::IDS_NOT_IMPL));
        }
        _ => {}
    }
}

/// SHBrowseForFolderW → picked path. None on cancel.
unsafe fn browse_for_folder(owner: HWND) -> Option<String> {
    let title = crate::i18n::to_wide(&crate::i18n::load_string(crate::i18n::IDS_BROWSE_PROMPT));
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

/// Show one view's controls and hide the other's.
unsafe fn show_view(dlg: HWND, tab: i32) {
    let library = tab == TAB_LIBRARY;
    for id in LIBRARY_VIEW {
        show(dlg, id, library);
    }
    for id in INSTALL_VIEW {
        show(dlg, id, !library);
    }
}

unsafe fn show(dlg: HWND, id: u16, visible: bool) {
    wm::ShowWindow(
        wm::GetDlgItem(dlg, id as i32),
        if visible { wm::SW_SHOW } else { wm::SW_HIDE },
    );
}

unsafe fn add_tab(tab: HWND, index: i32, string_id: u16) {
    let text = crate::i18n::to_wide(&crate::i18n::load_string(string_id));
    let mut item: windows_sys::Win32::UI::Controls::TCITEMW = core::mem::zeroed();
    item.mask = TCIF_TEXT;
    item.pszText = text.as_ptr() as *mut u16;
    wm::SendMessageW(
        tab,
        TCM_INSERTITEMW,
        index as WPARAM,
        &item as *const _ as LPARAM,
    );
}

unsafe fn add_column(list: HWND, index: i32, string_id: u16, width: i32) {
    let text = crate::i18n::to_wide(&crate::i18n::load_string(string_id));
    let mut col: windows_sys::Win32::UI::Controls::LVCOLUMNW = core::mem::zeroed();
    col.mask = LVCF_TEXT | LVCF_WIDTH;
    col.pszText = text.as_ptr() as *mut u16;
    col.cx = width;
    wm::SendMessageW(
        list,
        LVM_INSERTCOLUMNW,
        index as WPARAM,
        &col as *const _ as LPARAM,
    );
}

unsafe fn set_text(dlg: HWND, id: u16, string_id: u16) {
    let w = crate::i18n::to_wide(&crate::i18n::load_string(string_id));
    wm::SetDlgItemTextW(dlg, id as i32, w.as_ptr());
}

unsafe fn set_window_text(hwnd: HWND, text: &str) {
    let w = crate::i18n::to_wide(text);
    wm::SetWindowTextW(hwnd, w.as_ptr());
}

/// Write the shared status line.
pub unsafe fn status(dlg: HWND, text: &str) {
    let w = crate::i18n::to_wide(text);
    wm::SetDlgItemTextW(dlg, IDC_STATUS as i32, w.as_ptr());
}

unsafe fn set_dialog_icon(dlg: HWND) {
    let icon = wm::LoadIconW(crate::i18n::instance(), make_int_resource(IDI_APPICON));
    if !icon.is_null() {
        wm::SendMessageW(dlg, wm::WM_SETICON, ICON_BIG, icon as LPARAM);
        wm::SendMessageW(dlg, wm::WM_SETICON, ICON_SMALL, icon as LPARAM);
    }
}

/// Helper used by final error handling (CLAUDE.md: no silent failures).
pub fn message_box_error(text: &str) {
    let body = crate::i18n::to_wide(text);
    let title = crate::i18n::to_wide(&crate::i18n::load_string(crate::i18n::IDS_APP_TITLE));
    unsafe {
        wm::MessageBoxW(
            core::ptr::null_mut(),
            body.as_ptr(),
            title.as_ptr(),
            wm::MB_OK | wm::MB_ICONERROR,
        );
    }
}

/// Show an informational box (used by the --selftest-http flag in I3, which
/// has no console because this is a GUI-subsystem binary).
pub fn message_box_info(text: &str) {
    let body = crate::i18n::to_wide(text);
    let title = crate::i18n::to_wide(&crate::i18n::load_string(crate::i18n::IDS_APP_TITLE));
    unsafe {
        wm::MessageBoxW(
            core::ptr::null_mut(),
            body.as_ptr(),
            title.as_ptr(),
            wm::MB_OK | wm::MB_ICONINFORMATION,
        );
    }
}

// ---- Small helpers / constants windows-sys does not re-export here ----

const IDI_APPICON: u16 = 1;
const IDOK: u16 = 1;
const IDCANCEL: u16 = 2;

const CB_ADDSTRING: u32 = 0x0143;
const CB_SETCURSEL: u32 = 0x014E;
const CB_GETCURSEL: u32 = 0x0147;

const ICON_SMALL: WPARAM = 0;
const ICON_BIG: WPARAM = 1;

const BIF_RETURNONLYFSDIRS: u32 = 0x0000_0001;
const BIF_NEWDIALOGSTYLE: u32 = 0x0000_0040;

use windows_sys::Win32::UI::Controls::{
    LVCF_TEXT, LVCF_WIDTH, LVM_INSERTCOLUMNW, LVM_SETEXTENDEDLISTVIEWSTYLE, LVS_EX_CHECKBOXES,
    LVS_EX_FULLROWSELECT, LVS_EX_GRIDLINES, TCIF_TEXT, TCM_GETCURSEL, TCM_INSERTITEMW,
    TCN_SELCHANGE,
};

/// MAKEINTRESOURCEW — a resource id masquerading as a wide string pointer.
fn make_int_resource(id: u16) -> windows_sys::core::PCWSTR {
    id as usize as windows_sys::core::PCWSTR
}

fn loword(w: WPARAM) -> u16 {
    (w & 0xFFFF) as u16
}
