use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// Runs the wizard until the user finishes or cancels.
pub fn run(base_url: &str, cache_root: std::path::PathBuf) {
    let mut state = crate::wizard::state::WizardState::new(base_url.to_string(), cache_root);
    let state_pointer = &mut state as *mut _ as windows_sys::Win32::Foundation::LPARAM;

    let pages = [
        page(
            crate::wizard::control_ids::IDD_S1_LANGUAGE,
            Some(crate::wizard::scene::language_page),
            state_pointer,
        ),
        page(
            crate::wizard::control_ids::IDD_S2_MOTD,
            Some(crate::wizard::scene::motd_page),
            state_pointer,
        ),
        page(
            crate::wizard::control_ids::IDD_S3_FOLDER,
            Some(crate::wizard::scene::game_folder_page),
            state_pointer,
        ),
        page(
            crate::wizard::control_ids::IDD_S4_DOWNLOAD,
            Some(crate::wizard::scene::download_page),
            state_pointer,
        ),
        page(
            crate::wizard::control_ids::IDD_S4B_FETCH,
            Some(crate::wizard::scene::fetching_page),
            state_pointer,
        ),
        page(
            crate::wizard::control_ids::IDD_S5_CONFIG,
            Some(crate::wizard::scene::configuration_page),
            state_pointer,
        ),
        page(
            crate::wizard::control_ids::IDD_S6_INSTALL,
            Some(crate::wizard::scene::installation_page),
            state_pointer,
        ),
        page(
            crate::wizard::control_ids::IDD_S7_SUMMARY,
            Some(crate::wizard::scene::summary_page),
            state_pointer,
        ),
        // Last, because page order is what Next means: placed where it sits in the
        // story, Install walked onto it. It is reached and left by id.
        page(
            crate::wizard::control_ids::IDD_RESTORE,
            Some(crate::wizard::scene::restore_page),
            state_pointer,
        ),
    ];

    let caption = crate::i18n::to_wide(&crate::i18n::load_string(crate::i18n::IDS_APP_TITLE));
    unsafe {
        let mut header: ctl::PROPSHEETHEADERW_V2 = core::mem::zeroed();
        header.dwSize = core::mem::size_of::<ctl::PROPSHEETHEADERW_V2>() as u32;
        header.dwFlags = ctl::PSH_WIZARD | ctl::PSH_PROPSHEETPAGE | ctl::PSH_USEHICON;
        header.hInstance = crate::i18n::instance();
        header.pszCaption = caption.as_ptr();
        header.nPages = pages.len() as u32;
        header.Anonymous1.hIcon = crate::ui::load_app_icon();
        header.Anonymous2.nStartPage = 0;
        header.Anonymous3.ppsp = pages.as_ptr() as *mut ctl::PROPSHEETPAGEW;
        ctl::PropertySheetW(&mut header);
    }
}

/// A page from a dialog template in this executable, carrying the shared state as its `lParam`.
fn page(
    template: u16,
    dialog_proc: wm::DLGPROC,
    state: windows_sys::Win32::Foundation::LPARAM,
) -> ctl::PROPSHEETPAGEW {
    unsafe {
        let mut page: ctl::PROPSHEETPAGEW = core::mem::zeroed();
        page.dwSize = core::mem::size_of::<ctl::PROPSHEETPAGEW>() as u32;
        page.dwFlags = ctl::PSP_DEFAULT;
        page.hInstance = crate::i18n::instance();
        page.Anonymous1.pszTemplate = crate::ui::make_int_resource(template);
        page.pfnDlgProc = dialog_proc;
        page.lParam = state;
        page
    }
}
