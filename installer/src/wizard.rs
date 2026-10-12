//! The wizard: a comctl32 PropertySheet in wizard mode, one page per scene.
//!
//! Layout and behavior follow `installer/DESIGN.md` rev. 2. Using the OS
//! property sheet means Back/Next/Cancel, page switching and keyboard handling
//! cost no bytes of ours — which matters against the 500 kB gate.
//!
//! Scenes implemented so far: 1 Language, 2 Message of the day, 3 Game folder.
//! Scenes 4-7 (Download, Configuration, Installation, Summary) are task I2b's
//! remaining half and get appended to `PAGES`.
//!
//! RECORDED DEVIATION (DESIGN.md scene 1 says "small window"): a property
//! sheet is one size for every page, sized to the largest. Scene 1 therefore
//! shares the wizard frame rather than being a small dialog of its own. The
//! alternative — a separate modal before the sheet — would give scene 1 a
//! different Cancel behavior from every other scene.

#![allow(dead_code)]

use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

// ---- Control IDs (mirror res/installer.rc) ----
/// Shared by all three page templates: each page is its own dialog, so one
/// id can name the scene-title static everywhere.
pub const IDC_SCENE_TITLE: u16 = 299;

pub const IDD_S1_LANGUAGE: u16 = 300;
pub const IDC_S1_PROMPT: u16 = 301;
pub const IDC_S1_COMBO: u16 = 302;
pub const IDC_S1_NOTE: u16 = 303;

pub const IDD_S2_MOTD: u16 = 310;
pub const IDC_S2_MOTD: u16 = 311;
pub const IDC_S2_SECRET_LABEL: u16 = 312;
pub const IDC_S2_SECRET: u16 = 313;
pub const IDC_S2_STATUS: u16 = 314;

pub const IDD_S3_FOLDER: u16 = 320;
pub const IDD_S4_DOWNLOAD: u16 = 330;
pub const IDC_S4_TREE: u16 = 331;
pub const IDC_S4_STATUS: u16 = 332;
pub const IDC_S3_PATH_LABEL: u16 = 321;
pub const IDC_S3_PATH: u16 = 322;
pub const IDC_S3_BROWSE: u16 = 323;
pub const IDC_S3_GAME_LABEL: u16 = 324;
pub const IDC_S3_GAME: u16 = 325;

/// Where the dialog manager keeps a page's notification result. Not exported
/// by windows-sys; it is 0 by definition (the first pointer-sized slot of the
/// dialog's extra window memory).
const DWLP_MSGRESULT: i32 = 0;

/// Re-scan the folder this long after the last keystroke. Scanning on every
/// keystroke would stat a directory per character while a path is typed, and
/// would pop the "no game here" modal halfway through a valid path.
const DETECT_DEBOUNCE_MS: u32 = 400;
const TIMER_DETECT: usize = 1;

/// Everything the run accumulates, owned by the sheet and shared by the pages.
///
/// This replaces the per-game `App` of the rev. 1 ribbon layout: a run now
/// targets exactly one game, chosen on scene 3.
pub struct WizardState {
    pub language: crate::i18n::Language,
    /// Access key from scene 2. Empty means anonymous.
    pub secret: String,
    /// None until `GET /health` answers.
    pub online: Option<bool>,
    /// Set once the server accepted `secret`, so leaving and re-entering
    /// scene 2 does not re-validate an unchanged key.
    pub secret_checked: bool,
    pub folder: String,
    /// Games detected in `folder`, in `Game::ALL` order.
    pub detected: Vec<pacenotes_installer::gamedir::Game>,
    /// The one the user is installing for: the single detection, or their
    /// pick when the folder holds several games.
    pub game: Option<pacenotes_installer::gamedir::Game>,
    pub base_url: String,

    // ---- Scene 4 ----
    /// The Download tree's model. Rebuilt whenever the catalogue is fetched.
    pub tree: pacenotes_installer::tree::Tree,
    /// `HTREEITEM` per tree node, parallel to the model's node indices. UI
    /// state, kept here because a property-sheet page has no storage of its
    /// own beyond the one pointer it was handed.
    pub tree_items: Vec<isize>,
    /// Reentrancy guard: pushing model state back into the control raises the
    /// same change notification we act on, which would recurse.
    pub tree_syncing: bool,
    /// True once a catalogue fetch has been attempted, so re-entering the
    /// scene does not refetch.
    pub catalogue_loaded: bool,
    /// Keys the user committed with Download, in tree order.
    pub chosen: Vec<String>,

    /// Bumped per request so a late answer to a superseded one is dropped.
    request: usize,
}

impl WizardState {
    fn new(base_url: String) -> WizardState {
        WizardState {
            language: crate::i18n::Language::En,
            secret: String::new(),
            online: None,
            secret_checked: false,
            folder: String::new(),
            detected: Vec::new(),
            tree: pacenotes_installer::tree::Tree::default(),
            tree_items: Vec::new(),
            tree_syncing: false,
            catalogue_loaded: false,
            chosen: Vec::new(),
            game: None,
            base_url,
            request: 0,
        }
    }

    fn next_request(&mut self) -> usize {
        self.request += 1;
        self.request
    }
}

/// Run the wizard. Returns when the user finishes or cancels.
pub fn run(base_url: &str) {
    let mut state = WizardState::new(base_url.to_string());
    let state_ptr = &mut state as *mut WizardState as LPARAM;

    let pages = [
        page(IDD_S1_LANGUAGE, Some(s1_proc), state_ptr),
        page(IDD_S2_MOTD, Some(s2_proc), state_ptr),
        page(IDD_S3_FOLDER, Some(s3_proc), state_ptr),
        page(IDD_S4_DOWNLOAD, Some(s4_proc), state_ptr),
    ];

    let caption = crate::i18n::to_wide(&crate::i18n::load_string(crate::i18n::IDS_APP_TITLE));
    unsafe {
        let mut header: ctl::PROPSHEETHEADERW_V2 = core::mem::zeroed();
        header.dwSize = core::mem::size_of::<ctl::PROPSHEETHEADERW_V2>() as u32;
        // PSH_PROPSHEETPAGE: pages are templates we supply inline, not
        // pre-created HPROPSHEETPAGE handles.
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

/// One wizard page from a dialog template in our own resources.
fn page(template: u16, proc: wm::DLGPROC, state: LPARAM) -> ctl::PROPSHEETPAGEW {
    unsafe {
        let mut p: ctl::PROPSHEETPAGEW = core::mem::zeroed();
        p.dwSize = core::mem::size_of::<ctl::PROPSHEETPAGEW>() as u32;
        p.dwFlags = ctl::PSP_DEFAULT;
        p.hInstance = crate::i18n::instance();
        p.Anonymous1.pszTemplate = crate::ui::make_int_resource(template);
        p.pfnDlgProc = proc;
        p.lParam = state;
        p
    }
}

/// The shared state, stashed in the page's GWLP_USERDATA at WM_INITDIALOG.
///
/// Handed back as a reference rather than a raw pointer: every page would
/// otherwise dereference the pointer repeatedly in one expression, which is
/// how aliasing bugs start.
unsafe fn state<'a>(dlg: HWND) -> Option<&'a mut WizardState> {
    (wm::GetWindowLongPtrW(dlg, wm::GWLP_USERDATA) as *mut WizardState).as_mut()
}

/// WM_INITDIALOG hands a page a pointer to its PROPSHEETPAGEW, whose lParam is
/// what we passed in `page()`.
unsafe fn adopt_state<'a>(dlg: HWND, lparam: LPARAM) -> Option<&'a mut WizardState> {
    let psp = lparam as *const ctl::PROPSHEETPAGEW;
    let ptr = if psp.is_null() { 0 } else { (*psp).lParam };
    wm::SetWindowLongPtrW(dlg, wm::GWLP_USERDATA, ptr);
    (ptr as *mut WizardState).as_mut()
}

/// The sheet owning a page — the window that handles PSM_* messages.
unsafe fn sheet(dlg: HWND) -> HWND {
    wm::GetParent(dlg)
}

/// Enable exactly these wizard buttons.
unsafe fn set_buttons(dlg: HWND, flags: u32) {
    wm::SendMessageW(sheet(dlg), ctl::PSM_SETWIZBUTTONS, 0, flags as LPARAM);
}

/// comctl32's own ids for the wizard buttons, from prsht.h. Addressing them
/// directly is what makes per-scene labels work: PSM_SETNEXTTEXT is documented
/// for this but is silently ignored by the Wizard97 sheet — verified by
/// sending it from outside the process, where the caption did not change
/// either. Setting the control text does work.
const ID_WIZBACK: u16 = 0x3023;
const ID_WIZNEXT: u16 = 0x3024;
const ID_WIZFINISH: u16 = 0x3025;

/// Relabel the primary button for this scene (DESIGN.md: one label per scene).
///
/// Applied to both Next and Finish because the sheet swaps which of the two is
/// visible depending on PSM_SETWIZBUTTONS, and the last scene's primary button
/// is the Finish one.
unsafe fn set_next_text(dlg: HWND, string_id: u16) {
    let sheet = sheet(dlg);
    let text = crate::i18n::to_wide(&crate::i18n::load_string(string_id));
    wm::SetDlgItemTextW(sheet, ID_WIZNEXT as i32, text.as_ptr());
    wm::SetDlgItemTextW(sheet, ID_WIZFINISH as i32, text.as_ptr());

    // Back and Cancel come from comctl32 in the OS language, which would
    // leave two English buttons under a Polish wizard. DESIGN.md counts both
    // as scene chrome, so they follow the chosen language too.
    let back = crate::i18n::to_wide(&crate::i18n::load_string(crate::i18n::IDS_BTN_BACK));
    wm::SetDlgItemTextW(sheet, ID_WIZBACK as i32, back.as_ptr());
    let cancel = crate::i18n::to_wide(&crate::i18n::load_string(crate::i18n::IDS_BTN_CANCEL));
    wm::SetDlgItemTextW(sheet, crate::ui::IDCANCEL as i32, cancel.as_ptr());
}

/// The sheet's own title bar.
///
/// Set here rather than through PROPSHEETHEADERW::pszCaption, which this
/// Wizard97 sheet leaves empty; WM_SETTEXT on the sheet works.
unsafe fn set_sheet_caption(dlg: HWND) {
    let caption = crate::i18n::load_string(crate::i18n::IDS_APP_TITLE);
    crate::ui::set_window_text(sheet(dlg), &caption);
}

/// The scene title (DESIGN.md: every scene has a title).
///
/// RECORDED CHOICE: this is a static control in each page template, not the
/// Wizard97 header. The sheet is created with PSH_WIZARD (classic) rather than
/// PSH_WIZARD97 because the Wizard97 header strip renders its title only
/// alongside a header BITMAP — without one it reserved an empty band and
/// ignored both PROPSHEETPAGEW::pszHeaderTitle and PSM_SETHEADERTITLE, which
/// was verified on screen. Shipping a bitmap to satisfy it would cost size
/// budget for decoration. DESIGN.md allows this fallback.
///
/// Set per activation rather than baked into the template, because the pages
/// are described before scene 1 has asked which language to use.
unsafe fn set_header(dlg: HWND, string_id: u16) {
    crate::ui::set_text(dlg, IDC_SCENE_TITLE, string_id);
}

/// Answer a PSN_* notification. Returning TRUE from the proc plus a result in
/// DWLP_MSGRESULT is how a page talks back to the sheet.
unsafe fn reply(dlg: HWND, result: isize) -> isize {
    wm::SetWindowLongPtrW(dlg, DWLP_MSGRESULT, result);
    1
}

/// Refuse to leave the page.
unsafe fn stay(dlg: HWND) -> isize {
    reply(dlg, ctl::PSNRET_INVALID_NOCHANGEPAGE as isize)
}

/// Code of an incoming WM_NOTIFY, or None if it is not one.
unsafe fn notify_code(lparam: LPARAM) -> Option<u32> {
    let hdr = lparam as *const ctl::NMHDR;
    if hdr.is_null() {
        None
    } else {
        Some((*hdr).code)
    }
}

// ============================================================
// Scene 1 — Language
// ============================================================

unsafe extern "system" fn s1_proc(dlg: HWND, msg: u32, _w: WPARAM, lparam: LPARAM) -> isize {
    match msg {
        wm::WM_INITDIALOG => {
            let st = adopt_state(dlg, lparam);
            crate::ui::set_text(dlg, IDC_S1_PROMPT, crate::i18n::IDS_S1_PROMPT);
            crate::ui::set_text(dlg, IDC_S1_NOTE, crate::i18n::IDS_S1_RESTART);
            for lang in crate::i18n::Language::ALL {
                crate::ui::combo_add(dlg, IDC_S1_COMBO, lang.label());
            }
            // DESIGN.md: default to the OS UI language when we ship it.
            let initial = crate::i18n::os_language().unwrap_or(crate::i18n::Language::En);
            crate::ui::combo_select(dlg, IDC_S1_COMBO, initial.index());
            if let Some(st) = st {
                st.language = initial;
            }
            1
        }
        wm::WM_NOTIFY => match notify_code(lparam) {
            Some(ctl::PSN_SETACTIVE) => {
                set_sheet_caption(dlg);
                set_header(dlg, crate::i18n::IDS_S1_TITLE);
                // No Back: this is the first scene.
                set_buttons(dlg, ctl::PSWIZB_NEXT);
                set_next_text(dlg, crate::i18n::IDS_BTN_CONTINUE);
                reply(dlg, 0)
            }
            Some(ctl::PSN_WIZNEXT) => {
                if let Some(st) = state(dlg) {
                    let picked = crate::ui::combo_selection(dlg, IDC_S1_COMBO)
                        .map(crate::i18n::Language::from_index)
                        .unwrap_or(crate::i18n::Language::En);
                    st.language = picked;
                    // Applies to every page created from here on; scenes 2 and
                    // 3 have not been built yet, so they come up translated.
                    crate::i18n::set_language(picked);
                }
                reply(dlg, 0)
            }
            Some(ctl::PSN_QUERYCANCEL) => query_cancel(dlg),
            _ => 0,
        },
        _ => 0,
    }
}

// ============================================================
// Scene 2 — Message of the day
// ============================================================

unsafe extern "system" fn s2_proc(dlg: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    match msg {
        wm::WM_INITDIALOG => {
            adopt_state(dlg, lparam);
            crate::ui::set_text(dlg, IDC_S2_SECRET_LABEL, crate::i18n::IDS_S2_SECRET);
            crate::ui::set_text(dlg, IDC_S2_STATUS, crate::i18n::IDS_S2_CHECKING);
            // Show the offline copy immediately; the server's text replaces it
            // if one arrives. An empty pane while the network times out would
            // look broken.
            crate::ui::set_text(dlg, IDC_S2_MOTD, crate::i18n::IDS_S2_OFFLINE_MOTD);
            1
        }
        crate::worker::WM_WORKER_DONE => {
            let Some(message) = crate::worker::take(lparam) else {
                return 0;
            };
            let Some(st) = state(dlg) else { return 0 };
            if wparam != st.request {
                return 0; // Answer to a request we have moved past.
            }
            match message {
                crate::worker::Message::Health { online } => {
                    st.online = Some(online);
                    crate::ui::set_text(
                        dlg,
                        IDC_S2_STATUS,
                        if online {
                            crate::i18n::IDS_S2_ONLINE
                        } else {
                            crate::i18n::IDS_S2_OFFLINE
                        },
                    );
                }
                crate::worker::Message::Motd(Some(text)) => {
                    crate::ui::set_text_raw(dlg, IDC_S2_MOTD, &text);
                }
                crate::worker::Message::Motd(None) => { /* keep the offline copy */ }
                crate::worker::Message::SecretChecked(outcome) => {
                    on_secret_checked(dlg, st, outcome);
                }
                // Scene 4's, and it has its own handler.
                crate::worker::Message::Packages(_) => {}
            }
            1
        }
        wm::WM_NOTIFY => match notify_code(lparam) {
            Some(ctl::PSN_SETACTIVE) => {
                set_sheet_caption(dlg);
                set_header(dlg, crate::i18n::IDS_S2_TITLE);
                // DESIGN.md: Back only on scenes 3-5, so none here.
                set_buttons(dlg, ctl::PSWIZB_NEXT);
                set_next_text(dlg, crate::i18n::IDS_BTN_ACCEPT);
                start_scene2_probes(dlg);
                reply(dlg, 0)
            }
            Some(ctl::PSN_WIZNEXT) => on_accept(dlg),
            Some(ctl::PSN_QUERYCANCEL) => query_cancel(dlg),
            _ => 0,
        },
        _ => 0,
    }
}

/// Ask the server what it is and what it has to say. Both run on the worker;
/// the UI thread never blocks on the network.
unsafe fn start_scene2_probes(dlg: HWND) {
    let Some(st) = state(dlg) else { return };
    if st.online.is_some() {
        return; // Already answered; do not re-probe on every re-entry.
    }
    let id = st.next_request();
    let base = st.base_url.clone();
    crate::worker::spawn(dlg, id, move || crate::worker::check_health(base));

    let base = st.base_url.clone();
    let lang = st.language.tag();
    crate::worker::spawn(dlg, id, move || crate::worker::fetch_motd(base, lang));
}

/// ACCEPT pressed.
///
/// DESIGN.md: an empty secret continues anonymously; a secret while online is
/// validated first and a 401 keeps the user here; offline keeps the secret
/// unvalidated and moves on.
unsafe fn on_accept(dlg: HWND) -> isize {
    let Some(st) = state(dlg) else {
        return reply(dlg, 0);
    };
    let typed = crate::ui::get_text(dlg, IDC_S2_SECRET).trim().to_string();
    if typed != st.secret {
        st.secret = typed;
        st.secret_checked = false; // A changed key needs checking again.
    }
    if st.secret.is_empty() || st.secret_checked || st.online != Some(true) {
        return reply(dlg, 0);
    }

    // Validation is a network round trip, so navigation has to wait for the
    // answer: block the move, disable the buttons, and press Next ourselves
    // once the worker reports back.
    let id = st.next_request();
    let base = st.base_url.clone();
    let secret = st.secret.clone();
    set_buttons(dlg, 0);
    crate::worker::spawn(dlg, id, move || crate::worker::check_secret(base, secret));
    stay(dlg)
}

unsafe fn on_secret_checked(
    dlg: HWND,
    st: &mut WizardState,
    outcome: crate::worker::SecretOutcome,
) {
    set_buttons(dlg, ctl::PSWIZB_NEXT);
    match outcome {
        crate::worker::SecretOutcome::Rejected => {
            crate::ui::message_box_warning(
                dlg,
                &crate::i18n::load_string(crate::i18n::IDS_S2_BADSECRET),
            );
        }
        // Accepted, or we could not ask — DESIGN.md treats "could not ask" as
        // permission to continue with the key unvalidated.
        _ => {
            st.secret_checked = true;
            wm::SendMessageW(
                sheet(dlg),
                ctl::PSM_PRESSBUTTON,
                ctl::PSBTN_NEXT as WPARAM,
                0,
            );
        }
    }
}

// ============================================================
// Scene 3 — Game folder
// ============================================================

unsafe extern "system" fn s3_proc(dlg: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    match msg {
        wm::WM_INITDIALOG => {
            adopt_state(dlg, lparam);
            crate::ui::set_text(dlg, IDC_S3_PATH_LABEL, crate::i18n::IDS_S3_PATH);
            crate::ui::set_text(dlg, IDC_S3_GAME_LABEL, crate::i18n::IDS_S3_GAME);
            crate::ui::set_text(dlg, IDC_S3_BROWSE, crate::i18n::IDS_BTN_BROWSE);
            show_detection(dlg);
            1
        }
        wm::WM_COMMAND => {
            let id = crate::ui::loword(wparam);
            let code = crate::ui::hiword(wparam);
            if id == IDC_S3_PATH && code == EN_CHANGE {
                // Restart the debounce: the user is still typing.
                wm::SetTimer(dlg, TIMER_DETECT, DETECT_DEBOUNCE_MS, None);
            } else if id == IDC_S3_BROWSE && code == BN_CLICKED {
                on_browse(dlg);
            } else if id == IDC_S3_GAME && code == CBN_SELCHANGE {
                on_game_picked(dlg);
            }
            1
        }
        wm::WM_TIMER if wparam == TIMER_DETECT => {
            wm::KillTimer(dlg, TIMER_DETECT);
            // Typing never raises the modal — only Browse and Next do.
            redetect(dlg, false);
            1
        }
        wm::WM_NOTIFY => match notify_code(lparam) {
            Some(ctl::PSN_SETACTIVE) => {
                set_sheet_caption(dlg);
                set_header(dlg, crate::i18n::IDS_S3_TITLE);
                set_buttons(dlg, wizard_buttons(dlg));
                set_next_text(dlg, crate::i18n::IDS_BTN_NEXT);
                reply(dlg, 0)
            }
            Some(ctl::PSN_WIZNEXT) => {
                if state(dlg).and_then(|st| st.game).is_none() {
                    warn_no_game(dlg);
                    return stay(dlg);
                }
                reply(dlg, 0)
            }
            Some(ctl::PSN_QUERYCANCEL) => query_cancel(dlg),
            _ => 0,
        },
        _ => 0,
    }
}

/// Back is always available here; Next only once a game is settled.
unsafe fn wizard_buttons(dlg: HWND) -> u32 {
    if state(dlg).and_then(|st| st.game).is_some() {
        ctl::PSWIZB_BACK | ctl::PSWIZB_NEXT
    } else {
        ctl::PSWIZB_BACK
    }
}

unsafe fn on_browse(dlg: HWND) {
    let prompt = crate::i18n::load_string(crate::i18n::IDS_S3_PATH);
    let Some(picked) = crate::ui::browse_for_folder(dlg, &prompt) else {
        return;
    };
    crate::ui::set_text_raw(dlg, IDC_S3_PATH, &picked);
    // Browsing is a deliberate act, so an unusable choice is worth a modal
    // right away rather than silently leaving Next disabled.
    redetect(dlg, true);
}

/// Re-scan the folder and reflect it in the dropdown, the state and Next.
unsafe fn redetect(dlg: HWND, complain_if_none: bool) {
    let folder = crate::ui::get_text(dlg, IDC_S3_PATH);
    let detected = pacenotes_installer::gamedir::detect(&folder);
    let Some(st) = state(dlg) else { return };
    st.folder = folder;
    // Keep the previous pick if the folder still offers it; otherwise take the
    // first match, which is the whole answer when there is exactly one.
    st.game = match st.game {
        Some(prev) if detected.contains(&prev) => Some(prev),
        _ => detected.first().copied(),
    };
    st.detected = detected;
    let none_found = st.detected.is_empty();

    show_detection(dlg);
    set_buttons(dlg, wizard_buttons(dlg));

    if complain_if_none && none_found {
        warn_no_game(dlg);
    }
}

/// Dropdown contents per DESIGN.md scene 3: placeholder and disabled when
/// nothing is found, the single game shown but disabled when there is no
/// choice to make, and enabled listing only the matches when there is.
unsafe fn show_detection(dlg: HWND) {
    let (detected, chosen) = match state(dlg) {
        Some(st) => (st.detected.clone(), st.game),
        None => (Vec::new(), None),
    };

    crate::ui::combo_reset(dlg, IDC_S3_GAME);
    if detected.is_empty() {
        crate::ui::combo_add(
            dlg,
            IDC_S3_GAME,
            &crate::i18n::load_string(crate::i18n::IDS_S3_DETECTING),
        );
        crate::ui::combo_select(dlg, IDC_S3_GAME, 0);
        crate::ui::enable(dlg, IDC_S3_GAME, false);
        return;
    }
    for game in &detected {
        crate::ui::combo_add(dlg, IDC_S3_GAME, game.title());
    }
    let selected = chosen
        .and_then(|g| detected.iter().position(|d| *d == g))
        .unwrap_or(0);
    crate::ui::combo_select(dlg, IDC_S3_GAME, selected);
    // Nothing to choose when the folder holds exactly one game.
    crate::ui::enable(dlg, IDC_S3_GAME, detected.len() > 1);
}

unsafe fn on_game_picked(dlg: HWND) {
    let selection = crate::ui::combo_selection(dlg, IDC_S3_GAME);
    if let (Some(index), Some(st)) = (selection, state(dlg)) {
        st.game = st.detected.get(index).copied();
    }
    set_buttons(dlg, wizard_buttons(dlg));
}

unsafe fn warn_no_game(dlg: HWND) {
    crate::ui::message_box_warning(dlg, &crate::i18n::load_string(crate::i18n::IDS_S3_NOGAME));
}

// ============================================================
// Shared
// ============================================================

/// DESIGN.md: Cancel asks for confirmation. Returning TRUE from
/// PSN_QUERYCANCEL vetoes the close.
unsafe fn query_cancel(dlg: HWND) -> isize {
    let confirmed = crate::ui::confirm(
        dlg,
        &crate::i18n::load_string(crate::i18n::IDS_CANCEL_CONFIRM),
    );
    reply(dlg, isize::from(!confirmed))
}

// ---- Notification codes windows-sys does not re-export usefully ----
const EN_CHANGE: u16 = 0x0300;
const BN_CLICKED: u16 = 0;
const CBN_SELCHANGE: u16 = 1;

// ============================================================
// Scene 4 — Download
// ============================================================

unsafe extern "system" fn s4_proc(dlg: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    match msg {
        wm::WM_INITDIALOG => {
            adopt_state(dlg, lparam);
            // Partial checkboxes are an EXTENDED style, so the template cannot
            // ask for them.
            let tree = wm::GetDlgItem(dlg, IDC_S4_TREE as i32);
            wm::SendMessageW(
                tree,
                TVM_SETEXTENDEDSTYLE,
                ctl::TVS_EX_PARTIALCHECKBOXES as WPARAM,
                ctl::TVS_EX_PARTIALCHECKBOXES as LPARAM,
            );
            1
        }
        crate::worker::WM_WORKER_DONE => {
            let Some(message) = crate::worker::take(lparam) else {
                return 0;
            };
            let Some(st) = state(dlg) else { return 0 };
            if wparam != st.request {
                return 0; // answer to a request we have moved past
            }
            if let crate::worker::Message::Packages(result) = message {
                let (packages, status) = match result {
                    Ok(packages) => (packages, None),
                    // Not fatal: DESIGN.md says an unreachable server falls
                    // back to whatever is already on this computer.
                    Err(_) => (Vec::new(), Some(crate::i18n::IDS_S4_FETCH_FAILED)),
                };
                build_tree(dlg, st, packages, status);
            }
            1
        }
        wm::WM_NOTIFY => {
            let hdr = lparam as *const ctl::NMHDR;
            if hdr.is_null() {
                return 0;
            }
            match (*hdr).code {
                ctl::PSN_SETACTIVE => {
                    set_sheet_caption(dlg);
                    set_header(dlg, crate::i18n::IDS_S4_TITLE);
                    set_buttons(dlg, ctl::PSWIZB_BACK | ctl::PSWIZB_NEXT);
                    set_next_text(dlg, crate::i18n::IDS_BTN_DOWNLOAD);
                    start_catalogue_fetch(dlg);
                    reply(dlg, 0)
                }
                TVN_ITEMCHANGEDW => on_tree_item_changed(dlg, lparam),
                ctl::PSN_WIZNEXT => on_download(dlg),
                ctl::PSN_QUERYCANCEL => query_cancel(dlg),
                _ => 0,
            }
        }
        _ => 0,
    }
}

/// Ask the server for this game's catalogue, once per run.
unsafe fn start_catalogue_fetch(dlg: HWND) {
    let Some(st) = state(dlg) else { return };
    if st.catalogue_loaded {
        return;
    }
    st.catalogue_loaded = true;

    let Some(game) = st.game else {
        // Scene 3 does not let the user past without one, so this is only
        // reachable if that ever regresses.
        build_tree(dlg, st, Vec::new(), Some(crate::i18n::IDS_S4_EMPTY));
        return;
    };
    crate::ui::set_text(dlg, IDC_S4_STATUS, crate::i18n::IDS_S4_LOADING);

    if st.online != Some(true) {
        // Offline is a documented path, not a failure: build from the cache.
        build_tree(dlg, st, Vec::new(), Some(crate::i18n::IDS_S4_OFFLINE));
        return;
    }
    let id = st.next_request();
    let base = st.base_url.clone();
    let token = if st.secret.is_empty() {
        None
    } else {
        Some(st.secret.clone())
    };
    let game_id = game.id();
    crate::worker::spawn(dlg, id, move || {
        crate::worker::fetch_packages(base, token, game_id)
    });
}

/// Build the model from the catalogue plus the cache, then show it.
unsafe fn build_tree(
    dlg: HWND,
    st: &mut WizardState,
    packages: Vec<pacenotes_shared::PackageInfo>,
    status: Option<u16>,
) {
    let game = st
        .game
        .map(|g| g.id())
        .unwrap_or(pacenotes_shared::GAME_RBR);
    // TODO(I5): the real cache index. Until cache.rs lands nothing is
    // downloaded, so every leaf is NotCached and none start checked.
    let cached: Vec<pacenotes_installer::tree::CachedPackage> = Vec::new();
    st.tree = pacenotes_installer::tree::Tree::build(game, &packages, &cached);

    populate_tree(dlg, st);

    let status = status.unwrap_or(if st.tree.is_empty() {
        crate::i18n::IDS_S4_EMPTY
    } else {
        crate::i18n::IDS_S4_READY
    });
    crate::ui::set_text(dlg, IDC_S4_STATUS, status);
}

/// Mirror the model into the control. The node index goes in `lParam`, so a
/// notification maps straight back to the model.
unsafe fn populate_tree(dlg: HWND, st: &mut WizardState) {
    let tree = wm::GetDlgItem(dlg, IDC_S4_TREE as i32);
    st.tree_syncing = true;
    wm::SendMessageW(tree, TVM_DELETEITEM, 0, TVI_ROOT);
    st.tree_items = vec![0isize; st.tree.len()];

    let roots: Vec<usize> = st.tree.roots().to_vec();
    for root in roots {
        insert_node(tree, st, root, TVI_ROOT);
    }
    refresh_checks(dlg, st);
    st.tree_syncing = false;
}

unsafe fn insert_node(tree: HWND, st: &mut WizardState, node: usize, parent: isize) {
    let label = row_label(st, node);
    let text = crate::i18n::to_wide(&label);

    let mut item: ctl::TVINSERTSTRUCTW = core::mem::zeroed();
    item.hParent = parent;
    item.hInsertAfter = TVI_LAST;
    item.Anonymous.itemex.mask = ctl::TVIF_TEXT | ctl::TVIF_PARAM | ctl::TVIF_STATE;
    item.Anonymous.itemex.pszText = text.as_ptr() as *mut u16;
    item.Anonymous.itemex.lParam = node as LPARAM;
    // Grouping rows open, so the catalogue is visible at a glance.
    item.Anonymous.itemex.stateMask = ctl::TVIS_EXPANDED;
    item.Anonymous.itemex.state = if st.tree.node(node).is_leaf() {
        0
    } else {
        ctl::TVIS_EXPANDED
    };

    let handle = wm::SendMessageW(tree, TVM_INSERTITEMW, 0, &item as *const _ as LPARAM);
    st.tree_items[node] = handle;

    let children: Vec<usize> = st.tree.children(node).to_vec();
    for child in children {
        insert_node(tree, st, child, handle);
    }
}

/// The row's text: the model's label plus the localized state suffix. The
/// model holds no display strings, so the suffix is added here.
fn row_label(st: &WizardState, node: usize) -> String {
    let node = st.tree.node(node);
    let Some(state) = node.state else {
        return node.label.clone();
    };
    let suffix = match state {
        pacenotes_installer::tree::LeafState::NotCached => return node.label.clone(),
        pacenotes_installer::tree::LeafState::Downloaded => crate::i18n::IDS_STATE_DOWNLOADED,
        pacenotes_installer::tree::LeafState::UpdateAvailable => crate::i18n::IDS_STATE_UPDATE,
        pacenotes_installer::tree::LeafState::LocalOnly => crate::i18n::IDS_STATE_LOCAL,
    };
    format!("{} {}", node.label, crate::i18n::load_string(suffix))
}

/// Push every node's checkbox from the model into the control.
unsafe fn refresh_checks(dlg: HWND, st: &mut WizardState) {
    let tree = wm::GetDlgItem(dlg, IDC_S4_TREE as i32);
    let was_syncing = st.tree_syncing;
    st.tree_syncing = true;
    for node in 0..st.tree.len() {
        let handle = st.tree_items[node];
        if handle == 0 {
            continue;
        }
        // State image 1 = unchecked, 2 = checked, 3 = partial, and the index
        // lives in bits 12-15.
        let image: u32 = match st.tree.check_of(node) {
            pacenotes_installer::tree::Check::Unchecked => 1,
            pacenotes_installer::tree::Check::Checked => 2,
            pacenotes_installer::tree::Check::Partial => 3,
        };
        let mut item: ctl::TVITEMEXW = core::mem::zeroed();
        item.mask = ctl::TVIF_HANDLE | ctl::TVIF_STATE;
        item.hItem = handle;
        item.stateMask = ctl::TVIS_STATEIMAGEMASK;
        item.state = image << 12;
        wm::SendMessageW(tree, TVM_SETITEMW, 0, &item as *const _ as LPARAM);
    }
    st.tree_syncing = was_syncing;
}

/// The control toggled a checkbox: take it into the model, which propagates
/// through the subtree, then push the whole model back.
unsafe fn on_tree_item_changed(dlg: HWND, lparam: LPARAM) -> isize {
    let Some(st) = state(dlg) else { return 0 };
    if st.tree_syncing {
        return 0; // our own write coming back at us
    }
    let info = lparam as *const NmTvItemChange;
    if info.is_null() {
        return 0;
    }
    let node = (*info).lparam as usize;
    if node >= st.tree.len() {
        return 0;
    }
    // Bits 12-15 hold the state image: 2 is checked, anything else is not.
    let checked = (((*info).state_new & ctl::TVIS_STATEIMAGEMASK) >> 12) == 2;
    st.tree.set_checked(node, checked);
    refresh_checks(dlg, st);
    0
}

/// Download pressed: every category in the tree needs a pick, then the
/// selection is committed for scenes 5 and 6. No transfer happens here —
/// DESIGN.md keeps all of it on the one progress screen.
unsafe fn on_download(dlg: HWND) -> isize {
    let Some(st) = state(dlg) else {
        return reply(dlg, 0);
    };
    let missing = st.tree.categories_missing_a_pick();
    if !missing.is_empty() {
        let text = crate::i18n::load_string_arg(crate::i18n::IDS_S4_MISSING, &missing.join(", "));
        crate::ui::message_box_warning(dlg, &text);
        return stay(dlg);
    }
    st.chosen = st.tree.checked_keys();
    reply(dlg, 0)
}

// ---- Tree view bits windows-sys does not re-export usefully ----

const TVM_INSERTITEMW: u32 = 0x1132;
const TVM_DELETEITEM: u32 = 0x1101;
const TVM_SETITEMW: u32 = 0x113F;
const TVM_SETEXTENDEDSTYLE: u32 = 0x112C;
const TVI_ROOT: isize = -0x10000;
const TVI_LAST: isize = -0x0FFFE;
/// TVN_ITEMCHANGEDW. Not in windows-sys; TVN_FIRST (-400) - 17.
const TVN_ITEMCHANGEDW: u32 = (-417i32) as u32;

/// NMTVITEMCHANGE, which windows-sys does not declare.
#[repr(C)]
struct NmTvItemChange {
    hdr: ctl::NMHDR,
    change: u32,
    item: *mut core::ffi::c_void,
    state_new: u32,
    state_old: u32,
    lparam: LPARAM,
}
