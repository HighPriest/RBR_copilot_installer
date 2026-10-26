//! Scene 3's two routes, Download/Update and Configure/Install, as buttons in
//! the page: alternatives, not a primary and an afterthought, one click each.

use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// Neither route means anything until a game is settled.
pub unsafe fn show_route_buttons(dialog: HWND) {
    let ready = crate::wizard::page::state(dialog)
        .and_then(|state| state.game)
        .is_some();
    crate::ui::enable(dialog, crate::wizard::control_ids::IDC_S3_DOWNLOAD, ready);
    crate::ui::enable(dialog, crate::wizard::control_ids::IDC_S3_CONFIGURE, ready);
}

/// Starts the log in the game folder and jumps to the route's first page, by
/// page id, since only one of the two is the next page.
pub unsafe fn take_route(dialog: HWND, route: crate::wizard::state::Route) {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return;
    };
    if state.game.is_none() {
        super::game_folder::warn_no_game(dialog);
        return;
    }
    state.route = route;
    attach_log(dialog, state);

    let target = match route {
        crate::wizard::state::Route::Download => crate::wizard::control_ids::IDD_S4_DOWNLOAD,
        crate::wizard::state::Route::Configure => {
            // Scene 5 reads the tree, here the cache plus the ledger, ticking what is already here.
            super::download::load_tree(state, Vec::new());
            state.chosen = state.tree.checked_keys();
            let line = format!(
                "configuring from what is already here: {} package(s)",
                state.chosen.len()
            );
            super::progress_page::log_line(dialog, state, &line);
            crate::wizard::control_ids::IDD_S5_CONFIG
        }
    };
    wm::SendMessageW(
        crate::wizard::page::sheet(dialog),
        ctl::PSM_SETCURSELID,
        0,
        target as LPARAM,
    );
}

/// The log belongs with the install it describes, so it moves into the game
/// folder now, replaying what was said before.
unsafe fn attach_log(dialog: HWND, state: &mut crate::wizard::state::WizardState) {
    let folder = std::path::PathBuf::from(&state.folder);
    let stamp = pacenotes_installer::engine::utc_stamp(pacenotes_installer::engine::unix_now());
    state
        .log
        .attach(pacenotes_installer::runlog::log_path(&folder, &stamp));
    if let Some(game) = state.game {
        let line = format!("game: {} in {}", game.title(), state.folder);
        super::progress_page::log_line(dialog, state, &line);
    }
}
