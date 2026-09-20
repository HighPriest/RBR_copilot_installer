//! Pacenotes Installer — entry point.
//! Flow mirrors legacy Program.cs: language picker (Cancel = exit) → main dialog.
//! See installer/CLAUDE.md before editing. Size gate: check-size.ps1.
//!
//! Windows-only. The pure logic lives in the sibling lib target (src/lib.rs);
//! only the modules declared here may touch windows-sys.
#![windows_subsystem = "windows"]
#![cfg(windows)]

mod api;
mod http;
mod i18n;
mod ui;

fn main() {
    unsafe {
        let icc = windows_sys::Win32::UI::Controls::INITCOMMONCONTROLSEX {
            dwSize: core::mem::size_of::<windows_sys::Win32::UI::Controls::INITCOMMONCONTROLSEX>()
                as u32,
            dwICC: windows_sys::Win32::UI::Controls::ICC_STANDARD_CLASSES
                | windows_sys::Win32::UI::Controls::ICC_PROGRESS_CLASS
                | windows_sys::Win32::UI::Controls::ICC_LISTVIEW_CLASSES
                | windows_sys::Win32::UI::Controls::ICC_TAB_CLASSES,
        };
        windows_sys::Win32::UI::Controls::InitCommonControlsEx(&icc);
    }

    // Task I2: language picker dialog; on Cancel, exit (like Program.cs).
    let Some(lang) = ui::show_language_picker() else {
        return;
    };
    i18n::set_language(lang);

    // Task I2/I4: main dialog owns the app lifetime; worker thread posts
    // WM_APP progress messages back to it.
    ui::run_main_dialog();
}
