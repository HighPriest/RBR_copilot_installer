//! Pacenotes Installer: common controls, the developer self-test, then the wizard.
//! The Win32-free logic lives in the sibling library (`lib.rs`).
#![windows_subsystem = "windows"]
#![cfg(windows)]

mod api;
mod http;
mod i18n;
mod selftest_http;
mod ui;
mod wizard;
mod worker;

/// From `installer/.env`, read by build.rs; `--server <url>` overrides it, which
/// is how the tests reach a local server.
const DEFAULT_SERVER: &str = env!("PACENOTES_DEFAULT_SERVER");

fn main() {
    if let Some(selftest) = selftest_http::SelfTest::from_args() {
        std::process::exit(selftest.run());
    }

    init_common_controls();

    // Scene 1 is shown before the user can choose, so it starts in the OS language.
    if let Some(os_language) = i18n::os_language() {
        i18n::set_language(os_language);
    }

    wizard::run(&server_url());
}

fn init_common_controls() {
    let classes = windows_sys::Win32::UI::Controls::INITCOMMONCONTROLSEX {
        dwSize: core::mem::size_of::<windows_sys::Win32::UI::Controls::INITCOMMONCONTROLSEX>()
            as u32,
        dwICC: windows_sys::Win32::UI::Controls::ICC_STANDARD_CLASSES
            | windows_sys::Win32::UI::Controls::ICC_PROGRESS_CLASS
            | windows_sys::Win32::UI::Controls::ICC_LISTVIEW_CLASSES
            | windows_sys::Win32::UI::Controls::ICC_TAB_CLASSES
            | windows_sys::Win32::UI::Controls::ICC_TREEVIEW_CLASSES,
    };
    unsafe { windows_sys::Win32::UI::Controls::InitCommonControlsEx(&classes) };
}

fn server_url() -> String {
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--server" {
            if let Some(url) = arguments.next() {
                return url.trim_end_matches('/').to_string();
            }
        }
    }
    DEFAULT_SERVER.to_string()
}
