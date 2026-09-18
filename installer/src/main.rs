//! Pacenotes Installer — entry point.
//! Flow mirrors legacy Program.cs: language picker (Cancel = exit) → main dialog.
//! See installer/CLAUDE.md before editing. Size gate: check-size.ps1.
#![windows_subsystem = "windows"]
#![cfg(windows)]

mod api;
mod http;
mod i18n;
mod install;
mod sha256;
mod ui;
mod zip;

use windows_sys::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_PROGRESS_CLASS, ICC_STANDARD_CLASSES, ICC_LISTVIEW_CLASSES,
    INITCOMMONCONTROLSEX,
};

fn main() {
    unsafe {
        let icc = INITCOMMONCONTROLSEX {
            dwSize: core::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES | ICC_PROGRESS_CLASS | ICC_LISTVIEW_CLASSES,
        };
        InitCommonControlsEx(&icc);
    }

    // Task I2: language picker dialog; on Cancel, return (like Program.cs).
    let Some(lang) = ui::show_language_picker() else {
        return;
    };
    i18n::set_language(lang);

    // Task I2/I4: main dialog owns the app lifetime; worker thread posts
    // WM_APP progress messages back to it.
    ui::run_main_dialog();
}
