use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// Re-scanning on every keystroke would stat a folder per character and
/// complain halfway through a valid path.
const DETECT_DEBOUNCE_MS: u32 = 400;
const TIMER_DETECT: usize = 1;

const EN_CHANGE: u16 = 0x0300;
const BN_CLICKED: u16 = 0;
const CBN_SELCHANGE: u16 = 1;

/// Scene 3: the game folder, and which game in it when it holds several.
pub unsafe extern "system" fn game_folder_page(
    dialog: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match message {
        wm::WM_INITDIALOG => {
            crate::wizard::page::adopt_state(dialog, lparam);
            crate::ui::set_text(
                dialog,
                crate::wizard::control_ids::IDC_S3_PATH_LABEL,
                crate::i18n::IDS_S3_PATH,
            );
            crate::ui::set_text(
                dialog,
                crate::wizard::control_ids::IDC_S3_GAME_LABEL,
                crate::i18n::IDS_S3_GAME,
            );
            crate::ui::set_text(
                dialog,
                crate::wizard::control_ids::IDC_S3_BROWSE,
                crate::i18n::IDS_BTN_BROWSE,
            );
            show_detection(dialog);
            1
        }
        wm::WM_COMMAND => {
            on_command(dialog, wparam);
            1
        }
        wm::WM_TIMER if wparam == TIMER_DETECT => {
            wm::KillTimer(dialog, TIMER_DETECT);
            redetect(dialog, false);
            1
        }
        wm::WM_NOTIFY => match crate::wizard::page::notify_code(lparam) {
            Some(ctl::PSN_SETACTIVE) => {
                crate::wizard::page::set_sheet_caption(dialog);
                crate::wizard::page::set_header(dialog, crate::i18n::IDS_S3_TITLE);
                crate::wizard::page::set_buttons(dialog, wizard_buttons(dialog));
                crate::wizard::page::set_next_text(dialog, crate::i18n::IDS_BTN_NEXT);
                crate::wizard::page::reply(dialog, 0)
            }
            Some(ctl::PSN_WIZNEXT) => {
                if crate::wizard::page::state(dialog)
                    .and_then(|state| state.game)
                    .is_none()
                {
                    warn_no_game(dialog);
                    return crate::wizard::page::stay(dialog);
                }
                crate::wizard::page::reply(dialog, 0)
            }
            Some(ctl::PSN_QUERYCANCEL) => crate::wizard::page::query_cancel(dialog),
            _ => 0,
        },
        _ => 0,
    }
}

unsafe fn on_command(dialog: HWND, wparam: WPARAM) {
    let control = crate::ui::loword(wparam);
    let code = crate::ui::hiword(wparam);
    match (control, code) {
        (crate::wizard::control_ids::IDC_S3_PATH, EN_CHANGE) => {
            wm::SetTimer(dialog, TIMER_DETECT, DETECT_DEBOUNCE_MS, None);
        }
        (crate::wizard::control_ids::IDC_S3_BROWSE, BN_CLICKED) => on_browse(dialog),
        (crate::wizard::control_ids::IDC_S3_GAME, CBN_SELCHANGE) => on_game_picked(dialog),
        _ => {}
    }
}

/// Back always; Next only once a game is settled.
unsafe fn wizard_buttons(dialog: HWND) -> u32 {
    let has_game = crate::wizard::page::state(dialog)
        .and_then(|state| state.game)
        .is_some();
    if has_game {
        ctl::PSWIZB_BACK | ctl::PSWIZB_NEXT
    } else {
        ctl::PSWIZB_BACK
    }
}

/// Browsing is deliberate, so an unusable pick is reported at once.
unsafe fn on_browse(dialog: HWND) {
    let prompt = crate::i18n::load_string(crate::i18n::IDS_S3_PATH);
    let Some(picked) = crate::ui::browse_for_folder(dialog, &prompt) else {
        return;
    };
    crate::ui::set_text_raw(dialog, crate::wizard::control_ids::IDC_S3_PATH, &picked);
    redetect(dialog, true);
}

/// Keeps the previous game while the folder still holds it, else takes the first match.
unsafe fn redetect(dialog: HWND, complain_if_none: bool) {
    let folder = crate::ui::get_text(dialog, crate::wizard::control_ids::IDC_S3_PATH);
    let detected = pacenotes_installer::gamedir::detect(&folder);
    let Some(state) = crate::wizard::page::state(dialog) else {
        return;
    };
    state.folder = folder;
    state.game = match state.game {
        Some(previous) if detected.contains(&previous) => Some(previous),
        _ => detected.first().copied(),
    };
    state.detected = detected;
    let none_found = state.detected.is_empty();

    show_detection(dialog);
    crate::wizard::page::set_buttons(dialog, wizard_buttons(dialog));
    if complain_if_none && none_found {
        warn_no_game(dialog);
    }
}

/// A disabled placeholder when nothing is found, the one game disabled when
/// there is no choice, and only the matches enabled when there is.
unsafe fn show_detection(dialog: HWND) {
    let combo = crate::wizard::control_ids::IDC_S3_GAME;
    let (detected, chosen) = match crate::wizard::page::state(dialog) {
        Some(state) => (state.detected.clone(), state.game),
        None => (Vec::new(), None),
    };

    crate::ui::combo_reset(dialog, combo);
    if detected.is_empty() {
        crate::ui::combo_add(
            dialog,
            combo,
            &crate::i18n::load_string(crate::i18n::IDS_S3_DETECTING),
        );
        crate::ui::combo_select(dialog, combo, 0);
        crate::ui::enable(dialog, combo, false);
        return;
    }
    for game in &detected {
        crate::ui::combo_add(dialog, combo, game.title());
    }
    let selected = chosen
        .and_then(|chosen| detected.iter().position(|game| *game == chosen))
        .unwrap_or(0);
    crate::ui::combo_select(dialog, combo, selected);
    crate::ui::enable(dialog, combo, detected.len() > 1);
}

unsafe fn on_game_picked(dialog: HWND) {
    let selection = crate::ui::combo_selection(dialog, crate::wizard::control_ids::IDC_S3_GAME);
    if let (Some(index), Some(state)) = (selection, crate::wizard::page::state(dialog)) {
        state.game = state.detected.get(index).copied();
    }
    crate::wizard::page::set_buttons(dialog, wizard_buttons(dialog));
}

unsafe fn warn_no_game(dialog: HWND) {
    crate::ui::message_box_warning(
        dialog,
        &crate::i18n::load_string(crate::i18n::IDS_S3_NOGAME),
    );
}
