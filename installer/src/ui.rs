//! Win32 dialog procs and UI plumbing. Control IDs MUST match res/installer.rc.
//! Only this module + http.rs + main.rs may call windows-sys (see CLAUDE.md).
//! Layout and behavior follow installer/DESIGN.md. Tasks I2 / I2a.

#![allow(dead_code)]

use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

// ---- Control IDs (mirror res/installer.rc) ----
pub const IDD_LANGPICK: u16 = 100;
pub const IDC_LANGCOMBO: u16 = 101;
pub const IDC_LANGPROMPT: u16 = 102;
pub const IDC_LANGRESTART: u16 = 103;

pub const IDD_MAIN: u16 = 110;

pub const IDC_GAMETAB: u16 = 200;

pub const IDC_LBL_GAMEDIR: u16 = 210;
pub const IDC_GAMEDIR: u16 = 211;
pub const IDC_BROWSE: u16 = 212;
pub const IDC_DIRSTATUS: u16 = 213;

pub const IDC_LBL_INSTALLED: u16 = 220;
pub const IDC_LBL_I_VOICE: u16 = 221;
pub const IDC_I_VOICE: u16 = 222;
pub const IDC_LBL_I_UILANG: u16 = 223;
pub const IDC_I_UILANG: u16 = 224;
pub const IDC_LBL_I_STYLE: u16 = 225;
pub const IDC_I_STYLE: u16 = 226;
pub const IDC_INSTALL: u16 = 227;

pub const IDC_RIGHTTAB: u16 = 230;
pub const IDC_LBL_KEY: u16 = 240;
pub const IDC_KEY: u16 = 241;
pub const IDC_UNLOCK: u16 = 242;
pub const IDC_LBL_D_VOICE: u16 = 243;
pub const IDC_D_VOICE: u16 = 244;
pub const IDC_LBL_D_UILANG: u16 = 245;
pub const IDC_D_UILANG: u16 = 246;
pub const IDC_LBL_D_STYLE: u16 = 247;
pub const IDC_D_STYLE: u16 = 248;
pub const IDC_DOWNLOAD: u16 = 249;
pub const IDC_LBL_BACKUPS: u16 = 260;
pub const IDC_BACKUPS: u16 = 261;
pub const IDC_RESTORE: u16 = 262;

pub const IDC_PROGRESS: u16 = 270;
pub const IDC_STATUS: u16 = 271;

pub const IDC_INFOTAB: u16 = 280;
pub const IDC_NOTES: u16 = 281;
pub const IDC_LOGS: u16 = 282;

/// Games the installer can target. Proper nouns, deliberately untranslated.
/// DESIGN.md: every game tab shares one control set; only STATE differs.
pub const GAMES: [&str; 3] = ["Richard Burns Rally", "Dirt Rally", "BeamNG"];

/// Right-hand tab pages.
const RIGHT_DOWNLOAD: i32 = 0;
const RIGHT_BACKUPS: i32 = 1;
const DOWNLOAD_PAGE: [u16; 10] = [
    IDC_LBL_KEY,
    IDC_KEY,
    IDC_UNLOCK,
    IDC_LBL_D_VOICE,
    IDC_D_VOICE,
    IDC_LBL_D_UILANG,
    IDC_D_UILANG,
    IDC_LBL_D_STYLE,
    IDC_D_STYLE,
    IDC_DOWNLOAD,
];
const BACKUPS_PAGE: [u16; 3] = [IDC_LBL_BACKUPS, IDC_BACKUPS, IDC_RESTORE];

/// Bottom tab pages.
const INFO_NOTES: i32 = 0;
const INFO_LOGS: i32 = 1;

/// The three content categories a complete install needs one pick from.
const INSTALLED_LISTS: [u16; 3] = [IDC_I_VOICE, IDC_I_UILANG, IDC_I_STYLE];

// ---- Worker → UI messages (task I4). WPARAM/LPARAM payloads documented. ----
pub const WM_APP: u32 = 0x8000;
/// wparam = percent (0..=100), lparam = string-table id for status line.
pub const WM_WORKER_PROGRESS: u32 = WM_APP + 1;
/// lparam = Box<String> (leaked, reclaimed by receiver) for dynamic status.
pub const WM_WORKER_STATUS: u32 = WM_APP + 2;
pub const WM_WORKER_DONE: u32 = WM_APP + 3;
/// lparam = Box<String> error message for MessageBox.
pub const WM_WORKER_ERROR: u32 = WM_APP + 4;

/// Per-game state. Switching the ribbon saves the current game's state and
/// loads the next one's, so one control set serves all three games.
#[derive(Default, Clone)]
struct GameState {
    dir: String,
}

struct App {
    games: [GameState; GAMES.len()],
    current: usize,
    log: String,
}

impl App {
    fn new() -> Self {
        App {
            games: Default::default(),
            current: 0,
            log: String::new(),
        }
    }
}

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
            // The picker runs before a language is chosen, so it shows whatever
            // the OS UI language resolves to.
            set_text(dlg, IDC_LANGPROMPT, crate::i18n::IDS_LANG_PROMPT);
            set_text(dlg, IDC_LANGRESTART, crate::i18n::IDS_LANG_RESTART);
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
    // The App outlives the dialog proc's frames; reclaimed on WM_NCDESTROY.
    let app = Box::into_raw(Box::new(App::new()));
    unsafe {
        wm::DialogBoxParamW(
            crate::i18n::instance(),
            make_int_resource(IDD_MAIN),
            core::ptr::null_mut(),
            Some(main_proc),
            app as LPARAM,
        );
    }
}

/// The App pointer is parked in the dialog's GWLP_USERDATA slot.
unsafe fn app(dlg: HWND) -> *mut App {
    wm::GetWindowLongPtrW(dlg, wm::GWLP_USERDATA) as *mut App
}

unsafe extern "system" fn main_proc(dlg: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    match msg {
        wm::WM_INITDIALOG => {
            wm::SetWindowLongPtrW(dlg, wm::GWLP_USERDATA, lparam);
            init_main_dialog(dlg, &mut *(lparam as *mut App));
            1
        }
        wm::WM_NOTIFY => {
            let hdr = lparam as *const windows_sys::Win32::UI::Controls::NMHDR;
            if !hdr.is_null() && (*hdr).code == TCN_SELCHANGE {
                let state = app(dlg);
                if state.is_null() {
                    return 0;
                }
                match (*hdr).idFrom as u16 {
                    IDC_GAMETAB => on_game_change(dlg, &mut *state),
                    IDC_RIGHTTAB => show_right_page(dlg, tab_sel(dlg, IDC_RIGHTTAB)),
                    IDC_INFOTAB => show_info_page(dlg, tab_sel(dlg, IDC_INFOTAB)),
                    _ => {}
                }
            }
            0
        }
        wm::WM_COMMAND => {
            let state = app(dlg);
            if !state.is_null() {
                on_command(dlg, &mut *state, loword(wparam), hiword(wparam));
            }
            1
        }
        wm::WM_CLOSE => {
            wm::EndDialog(dlg, 0);
            1
        }
        wm::WM_NCDESTROY => {
            let state = app(dlg);
            if !state.is_null() {
                drop(Box::from_raw(state));
                wm::SetWindowLongPtrW(dlg, wm::GWLP_USERDATA, 0);
            }
            0
        }
        _ => 0,
    }
}

unsafe fn init_main_dialog(dlg: HWND, state: &mut App) {
    set_dialog_icon(dlg);
    set_window_text(dlg, &crate::i18n::load_string(crate::i18n::IDS_APP_TITLE));

    // Every tab control must sit BELOW its page controls in the Z-order.
    // Controls declared earlier in a dialog template are HIGHER in Z-order,
    // so without this each tab body would paint over its own pages.
    //
    // Order matters: each call drops that control to the VERY bottom, so the
    // last one pushed ends up lowest. The game ribbon contains the other two
    // tab controls, so it must be pushed LAST or its body hides their headers.
    for id in [IDC_INFOTAB, IDC_RIGHTTAB, IDC_GAMETAB] {
        wm::SetWindowPos(
            wm::GetDlgItem(dlg, id as i32),
            wm::HWND_BOTTOM,
            0,
            0,
            0,
            0,
            wm::SWP_NOMOVE | wm::SWP_NOSIZE | wm::SWP_NOACTIVATE,
        );
    }

    // Game ribbon across the top (DESIGN.md).
    let gametab = wm::GetDlgItem(dlg, IDC_GAMETAB as i32);
    for (i, name) in GAMES.iter().enumerate() {
        add_tab_text(gametab, i as i32, name);
    }
    let righttab = wm::GetDlgItem(dlg, IDC_RIGHTTAB as i32);
    add_tab(righttab, RIGHT_DOWNLOAD, crate::i18n::IDS_TAB_DOWNLOAD);
    add_tab(righttab, RIGHT_BACKUPS, crate::i18n::IDS_TAB_BACKUPS);
    let infotab = wm::GetDlgItem(dlg, IDC_INFOTAB as i32);
    add_tab(infotab, INFO_NOTES, crate::i18n::IDS_TAB_NOTES);
    add_tab(infotab, INFO_LOGS, crate::i18n::IDS_TAB_LOGS);

    // Localize every caption; the .rc text is only an English fallback.
    set_text(dlg, IDC_LBL_GAMEDIR, crate::i18n::IDS_LBL_GAMEDIR);
    set_text(dlg, IDC_BROWSE, crate::i18n::IDS_BTN_BROWSE);
    set_text(dlg, IDC_LBL_INSTALLED, crate::i18n::IDS_LBL_INSTALLED);
    set_text(dlg, IDC_LBL_I_VOICE, crate::i18n::IDS_CAT_VOICE);
    set_text(dlg, IDC_LBL_I_UILANG, crate::i18n::IDS_CAT_UILANG);
    set_text(dlg, IDC_LBL_I_STYLE, crate::i18n::IDS_CAT_STYLE);
    set_text(dlg, IDC_INSTALL, crate::i18n::IDS_BTN_INSTALL);
    set_text(dlg, IDC_LBL_KEY, crate::i18n::IDS_LBL_ACCESSKEY);
    set_text(dlg, IDC_UNLOCK, crate::i18n::IDS_BTN_UNLOCK);
    set_text(dlg, IDC_LBL_D_VOICE, crate::i18n::IDS_CAT_VOICE);
    set_text(dlg, IDC_LBL_D_UILANG, crate::i18n::IDS_CAT_UILANG);
    set_text(dlg, IDC_LBL_D_STYLE, crate::i18n::IDS_CAT_STYLE);
    set_text(dlg, IDC_DOWNLOAD, crate::i18n::IDS_BTN_DOWNLOAD);
    set_text(dlg, IDC_LBL_BACKUPS, crate::i18n::IDS_LBL_BACKUPS);
    set_text(dlg, IDC_RESTORE, crate::i18n::IDS_BTN_RESTORE);
    set_text(dlg, IDC_NOTES, crate::i18n::IDS_NOTES_BODY);
    set_text(dlg, IDCANCEL, crate::i18n::IDS_BTN_CLOSE);

    status(dlg, &crate::i18n::load_string(crate::i18n::IDS_READY));
    log(
        dlg,
        state,
        &crate::i18n::load_string(crate::i18n::IDS_LOG_STARTED),
    );

    show_right_page(dlg, RIGHT_DOWNLOAD);
    show_info_page(dlg, INFO_NOTES);
    load_game(dlg, state);
}

/// Ribbon changed: persist the outgoing game's fields, then show the new one's.
unsafe fn on_game_change(dlg: HWND, state: &mut App) {
    save_game(dlg, state);
    state.current = tab_sel(dlg, IDC_GAMETAB).clamp(0, GAMES.len() as i32 - 1) as usize;
    load_game(dlg, state);
}

unsafe fn save_game(dlg: HWND, state: &mut App) {
    state.games[state.current].dir = get_text(dlg, IDC_GAMEDIR);
}

unsafe fn load_game(dlg: HWND, state: &mut App) {
    let dir = state.games[state.current].dir.clone();
    set_window_text(wm::GetDlgItem(dlg, IDC_GAMEDIR as i32), &dir);
    validate_dir(dlg);
}

/// DESIGN.md: "the field should be validated".
unsafe fn validate_dir(dlg: HWND) -> bool {
    let dir = get_text(dlg, IDC_GAMEDIR);
    let id = if dir.trim().is_empty() {
        crate::i18n::IDS_DIR_EMPTY
    } else if std::path::Path::new(dir.trim()).is_dir() {
        crate::i18n::IDS_DIR_OK
    } else {
        crate::i18n::IDS_DIR_INVALID
    };
    set_text(dlg, IDC_DIRSTATUS, id);
    id == crate::i18n::IDS_DIR_OK
}

unsafe fn on_command(dlg: HWND, state: &mut App, id: u16, code: u16) {
    // Re-validate as the user types the folder path.
    if id == IDC_GAMEDIR && code == EN_CHANGE {
        validate_dir(dlg);
        return;
    }
    if code != BN_CLICKED && !matches!(id, IDCANCEL | IDOK) {
        return;
    }

    match id {
        IDCANCEL => {
            wm::EndDialog(dlg, 0);
        }
        IDC_BROWSE => {
            if let Some(dir) = browse_for_folder(dlg) {
                set_window_text(wm::GetDlgItem(dlg, IDC_GAMEDIR as i32), &dir);
                save_game(dlg, state);
                validate_dir(dlg);
            }
        }
        IDC_INSTALL => on_install(dlg, state),
        // These need the cache/engine work in I5-I7. Until then say so on the
        // status line rather than failing silently.
        IDC_DOWNLOAD | IDC_UNLOCK | IDC_RESTORE => {
            status(dlg, &crate::i18n::load_string(crate::i18n::IDS_NOT_IMPL));
        }
        _ => {}
    }
}

/// DESIGN.md: Install "checks if item from each category has been pressed and
/// then proceeds with installation". Until I7 there is nothing to proceed
/// with, but the preconditions are real and enforced here.
unsafe fn on_install(dlg: HWND, state: &mut App) {
    if !validate_dir(dlg) {
        report_error(dlg, state, crate::i18n::IDS_ERR_NO_DIR);
        return;
    }
    let all_picked = INSTALLED_LISTS
        .iter()
        .all(|&list| wm::SendDlgItemMessageW(dlg, list as i32, LB_GETCURSEL, 0, 0) >= 0);
    if !all_picked {
        report_error(dlg, state, crate::i18n::IDS_ERR_NO_PICK);
        return;
    }
    status(dlg, &crate::i18n::load_string(crate::i18n::IDS_NOT_IMPL));
}

/// DESIGN.md: "If an error happens, then the window should automatically
/// switch to the logs tab, to inform the user about the error."
unsafe fn report_error(dlg: HWND, state: &mut App, string_id: u16) {
    let text = crate::i18n::load_string(string_id);
    log(dlg, state, &text);
    status(dlg, &text);
    set_tab_sel(dlg, IDC_INFOTAB, INFO_LOGS);
    show_info_page(dlg, INFO_LOGS);
}

/// Append a line to the log pane. Prefixed with the active game so entries
/// stay meaningful once the ribbon is in use.
unsafe fn log(dlg: HWND, state: &mut App, line: &str) {
    if !state.log.is_empty() {
        state.log.push_str("\r\n");
    }
    state
        .log
        .push_str(&format!("[{}] {line}", GAMES[state.current]));
    let logs = wm::GetDlgItem(dlg, IDC_LOGS as i32);
    set_window_text(logs, &state.log);
    // Keep the newest line in view.
    wm::SendMessageW(logs, EM_LINESCROLL, 0, i16::MAX as LPARAM);
}

unsafe fn show_right_page(dlg: HWND, page: i32) {
    for id in DOWNLOAD_PAGE {
        show(dlg, id, page == RIGHT_DOWNLOAD);
    }
    for id in BACKUPS_PAGE {
        show(dlg, id, page == RIGHT_BACKUPS);
    }
}

unsafe fn show_info_page(dlg: HWND, page: i32) {
    show(dlg, IDC_NOTES, page == INFO_NOTES);
    show(dlg, IDC_LOGS, page == INFO_LOGS);
}

/// SHBrowseForFolderW → picked path. None on cancel.
unsafe fn browse_for_folder(owner: HWND) -> Option<String> {
    let title = crate::i18n::to_wide(&crate::i18n::load_string(crate::i18n::IDS_LBL_GAMEDIR));
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

// ---- Small Win32 helpers ----

unsafe fn show(dlg: HWND, id: u16, visible: bool) {
    wm::ShowWindow(
        wm::GetDlgItem(dlg, id as i32),
        if visible { wm::SW_SHOW } else { wm::SW_HIDE },
    );
}

unsafe fn tab_sel(dlg: HWND, id: u16) -> i32 {
    wm::SendMessageW(wm::GetDlgItem(dlg, id as i32), TCM_GETCURSEL, 0, 0) as i32
}

unsafe fn set_tab_sel(dlg: HWND, id: u16, index: i32) {
    wm::SendMessageW(
        wm::GetDlgItem(dlg, id as i32),
        TCM_SETCURSEL,
        index as WPARAM,
        0,
    );
}

unsafe fn add_tab(tab: HWND, index: i32, string_id: u16) {
    add_tab_text(tab, index, &crate::i18n::load_string(string_id));
}

unsafe fn add_tab_text(tab: HWND, index: i32, text: &str) {
    let w = crate::i18n::to_wide(text);
    let mut item: windows_sys::Win32::UI::Controls::TCITEMW = core::mem::zeroed();
    item.mask = TCIF_TEXT;
    item.pszText = w.as_ptr() as *mut u16;
    wm::SendMessageW(
        tab,
        TCM_INSERTITEMW,
        index as WPARAM,
        &item as *const _ as LPARAM,
    );
}

unsafe fn set_text(dlg: HWND, id: u16, string_id: u16) {
    let w = crate::i18n::to_wide(&crate::i18n::load_string(string_id));
    wm::SetDlgItemTextW(dlg, id as i32, w.as_ptr());
}

unsafe fn get_text(dlg: HWND, id: u16) -> String {
    let mut buf = [0u16; 512];
    let len = wm::GetDlgItemTextW(dlg, id as i32, buf.as_mut_ptr(), buf.len() as i32);
    String::from_utf16_lossy(&buf[..len as usize])
}

unsafe fn set_window_text(hwnd: HWND, text: &str) {
    let w = crate::i18n::to_wide(text);
    wm::SetWindowTextW(hwnd, w.as_ptr());
}

/// Write the shared status line (DESIGN.md: current file + size during work).
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
    message_box(text, wm::MB_ICONERROR);
}

/// Informational box; used where there is no console (GUI-subsystem binary).
pub fn message_box_info(text: &str) {
    message_box(text, wm::MB_ICONINFORMATION);
}

fn message_box(text: &str, icon: u32) {
    let body = crate::i18n::to_wide(text);
    let title = crate::i18n::to_wide(&crate::i18n::load_string(crate::i18n::IDS_APP_TITLE));
    unsafe {
        wm::MessageBoxW(
            core::ptr::null_mut(),
            body.as_ptr(),
            title.as_ptr(),
            wm::MB_OK | icon,
        );
    }
}

// ---- Constants windows-sys does not re-export in a usable place ----

const IDI_APPICON: u16 = 1;
const IDOK: u16 = 1;
const IDCANCEL: u16 = 2;

const CB_ADDSTRING: u32 = 0x0143;
const CB_SETCURSEL: u32 = 0x014E;
const CB_GETCURSEL: u32 = 0x0147;
const LB_GETCURSEL: u32 = 0x0188;
const EM_LINESCROLL: u32 = 0x00B6;

const BN_CLICKED: u16 = 0;
const EN_CHANGE: u16 = 0x0300;

const ICON_SMALL: WPARAM = 0;
const ICON_BIG: WPARAM = 1;

const BIF_RETURNONLYFSDIRS: u32 = 0x0000_0001;
const BIF_NEWDIALOGSTYLE: u32 = 0x0000_0040;

use windows_sys::Win32::UI::Controls::{
    TCIF_TEXT, TCM_GETCURSEL, TCM_INSERTITEMW, TCM_SETCURSEL, TCN_SELCHANGE,
};

/// MAKEINTRESOURCEW — a resource id masquerading as a wide string pointer.
fn make_int_resource(id: u16) -> windows_sys::core::PCWSTR {
    id as usize as windows_sys::core::PCWSTR
}

fn loword(w: WPARAM) -> u16 {
    (w & 0xFFFF) as u16
}

fn hiword(w: WPARAM) -> u16 {
    ((w >> 16) & 0xFFFF) as u16
}
