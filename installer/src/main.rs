//! Pacenotes Installer: common controls, the developer self-test, then the wizard.
//! The Win32-free logic lives in the sibling library (`lib.rs`).
#![windows_subsystem = "windows"]
#![cfg(windows)]

mod api;
mod crash;
mod http;
mod i18n;
mod package_download;
mod selftest;
mod single_instance;
mod ui;
mod wizard;
mod worker;

/// From `installer/.env`, read by build.rs; `--server <url>` overrides it, which
/// is how the tests reach a local server.
const DEFAULT_SERVER: &str = env!("PACENOTES_DEFAULT_SERVER");

fn main() {
    crash::install_panic_hook();

    if let Some(selftest) = selftest::HttpSelfTest::from_args() {
        std::process::exit(selftest.run());
    }
    if let Some(selftest) = selftest::DownloadSelfTest::from_args() {
        std::process::exit(selftest.run());
    }
    if let Some(selftest) = selftest::ConflictReportSelfTest::from_args() {
        std::process::exit(selftest.run());
    }
    selftest::panic_if_requested();

    init_common_controls();

    // Scene 1 is shown before the user can choose, so it starts in the OS language.
    if let Some(os_language) = i18n::os_language() {
        i18n::set_language(os_language);
    }

    if single_instance::another_instance_running() {
        ui::message_box_warning(
            core::ptr::null_mut(),
            &i18n::load_string(i18n::IDS_ALREADY_RUNNING),
        );
        return;
    }

    wizard::run(&server_url(), cache_root());
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
    argument_after("--server")
        .map(|url| url.trim_end_matches('/').to_string())
        .unwrap_or_else(|| DEFAULT_SERVER.to_string())
}

/// `--cache <dir>` gives a test run a cache of its own; otherwise the per-user
/// cache, or a throwaway one in the temp folder when that cannot be located.
fn cache_root() -> std::path::PathBuf {
    argument_after("--cache")
        .map(std::path::PathBuf::from)
        .or_else(pacenotes_installer::cache::default_root)
        .unwrap_or_else(|| {
            std::env::temp_dir()
                .join("PacenotesInstaller")
                .join("cache")
        })
}

fn argument_after(flag: &str) -> Option<String> {
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == flag {
            return arguments.next();
        }
    }
    None
}
