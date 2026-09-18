//! Win32 dialog procs and UI plumbing. Control IDs MUST match res/installer.rc.
//! Only this module + http.rs + main.rs may call windows-sys (see CLAUDE.md).

#![allow(dead_code)] // scaffolding

// ---- Control IDs (mirror res/installer.rc) ----
pub const IDD_LANGPICK: u16 = 100;
pub const IDC_LANGCOMBO: u16 = 101;
pub const IDD_MAIN: u16 = 110;
pub const IDC_PKGLIST: u16 = 111;
pub const IDC_TARGETDIR: u16 = 112;
pub const IDC_BROWSE: u16 = 113;
pub const IDC_TOKEN: u16 = 114;
pub const IDC_UNLOCK: u16 = 115;
pub const IDC_PROGRESS: u16 = 116;
pub const IDC_STATUS: u16 = 117;
pub const IDC_INSTALL: u16 = 118;
pub const IDC_UNINSTALL: u16 = 119;

// ---- Worker → UI messages (task I4). WPARAM/LPARAM payloads documented. ----
pub const WM_APP: u32 = 0x8000;
/// wparam = percent (0..=100), lparam = string-table id for status line.
pub const WM_WORKER_PROGRESS: u32 = WM_APP + 1;
/// lparam = Box<String> (leaked, reclaimed by receiver) for dynamic status.
pub const WM_WORKER_STATUS: u32 = WM_APP + 2;
pub const WM_WORKER_DONE: u32 = WM_APP + 3;
/// lparam = Box<String> error message for MessageBox.
pub const WM_WORKER_ERROR: u32 = WM_APP + 4;

/// Task I2: DialogBoxParamW(IDD_LANGPICK, langpick_proc). Returns None on Cancel.
pub fn show_language_picker() -> Option<crate::i18n::Language> {
    // TODO(I2): implement dialog; scaffolding defaults to English.
    Some(crate::i18n::Language::En)
}

/// Task I2/I4: DialogBoxParamW(IDD_MAIN, main_proc); pumps until close.
pub fn run_main_dialog() {
    // TODO(I2): implement. Steps:
    //  - WM_INITDIALOG: populate listview columns, spawn manifest fetch worker
    //  - IDC_BROWSE: SHBrowseForFolderW → set IDC_TARGETDIR
    //  - IDC_UNLOCK: read token, worker re-fetch manifest with bearer
    //  - IDC_INSTALL: spawn install pipeline worker (install.rs)
    //  - WM_WORKER_*: update progress bar / status / message boxes
}

/// Helper used by final error handling (CLAUDE.md: no silent failures).
pub fn message_box_error(_text: &str) {
    // TODO(I2): MessageBoxW(MB_ICONERROR)
}
