//! Scene 3's three routes, Download/Update, Configure/Install and
//! Restore/Uninstall, as buttons in the page: alternatives, not a primary and an
//! afterthought, one click each.

use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

// No route means anything until a game is settled.
pub unsafe fn show_route_buttons(dialog: HWND) {
    let ready = crate::wizard::page::state(dialog)
        .and_then(|state| state.game)
        .is_some();
    crate::ui::enable(dialog, crate::wizard::control_ids::IDC_S3_DOWNLOAD, ready);
    crate::ui::enable(dialog, crate::wizard::control_ids::IDC_S3_CONFIGURE, ready);
    crate::ui::enable(dialog, crate::wizard::control_ids::IDC_S3_RESTORE, ready);
}

/// Starts the log and opens the cache in the game folder, then jumps to the
/// route's first page by page id, since at most one of them is the next page.
pub unsafe fn take_route(dialog: HWND, route: crate::wizard::state::Route) {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return;
    };
    if state.game.is_none() {
        super::game_folder::warn_no_game(dialog);
        return;
    }
    // Every route writes into the folder, and this is where the user commits to it.
    let folder = std::path::PathBuf::from(&state.folder);
    if let Err(reason) = pacenotes_installer::engine::check_writable(&folder) {
        refuse_unwritable(dialog, state, &reason.to_string());
        return;
    }
    state.route = route;
    // Both belong to the folder, and this is the moment it is settled.
    attach_log(dialog, state);
    open_cache(state);

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
        // Reads the game folder, not the server, so the way back works when nothing else does.
        crate::wizard::state::Route::Restore => crate::wizard::control_ids::IDD_RESTORE,
    };
    wm::SendMessageW(
        crate::wizard::page::sheet(dialog),
        ctl::PSM_SETCURSELID,
        0,
        target as LPARAM,
    );
}

/// Says why, and offers to restart as administrator with what this run already
/// knows (P11); already elevated, there is nothing to offer, since asking again
/// would loop. The user stays on scene 3 unless the restart starts.
unsafe fn refuse_unwritable(
    dialog: HWND,
    state: &mut crate::wizard::state::WizardState,
    reason: &str,
) {
    let line = format!("the game folder cannot be written to: {reason}");
    super::progress_page::log_line(dialog, state, &line);
    if crate::elevation::is_elevated() {
        let text =
            crate::i18n::load_string_arg(crate::i18n::IDS_S3_NOT_WRITABLE_ADMIN, &state.folder);
        crate::ui::message_box_warning(dialog, &text);
        return;
    }
    let question = crate::i18n::load_string_arg(crate::i18n::IDS_S3_NOT_WRITABLE, &state.folder);
    if !crate::ui::confirm(dialog, &question) {
        return;
    }
    let restart = crate::elevation::Restart {
        language: state.language,
        folder: state.folder.clone(),
        secret: state.secret.clone(),
    };
    if crate::elevation::restart_elevated(crate::wizard::page::sheet(dialog), &restart) {
        // The elevated instance takes over, and waits for this one to be gone.
        std::process::exit(0);
    }
    super::progress_page::log_line(dialog, state, "restarting as administrator was cancelled");
}

/// Opened per route taken, so a user who corrects the folder and starts again
/// does not read the previous folder's downloads.
fn open_cache(state: &mut crate::wizard::state::WizardState) {
    let root = state.cache_override.clone().unwrap_or_else(|| {
        pacenotes_installer::cache::dir_for(std::path::Path::new(&state.folder))
    });
    state.cache = pacenotes_installer::cache::Cache::open(root);
    // An override may serve other folders, whose needs this one cannot see.
    if state.cache_override.is_none() {
        sweep_cache(state);
    }
}

/// Removes the downloads nothing in this folder needs any more (I9), which
/// only the folder itself can tell: the restore points need older versions
/// the cache index has already forgotten.
fn sweep_cache(state: &mut crate::wizard::state::WizardState) {
    let in_use = pacenotes_installer::restore::payloads_in_use(std::path::Path::new(&state.folder));
    if in_use.is_none() {
        state
            .log
            .line("cache: a record here could not be read, so no download was removed");
    }
    let swept = state.cache.sweep(in_use.as_deref());
    if swept.files > 0 {
        state.log.line(format!(
            "cache: removed {} file(s) nothing needs any more, {}",
            swept.files,
            pacenotes_installer::size::human(swept.bytes)
        ));
    }
}

/// The log belongs with the install it describes, so it moves into the game
/// folder now, replaying what was said before.
unsafe fn attach_log(dialog: HWND, state: &mut crate::wizard::state::WizardState) {
    let folder = std::path::PathBuf::from(&state.folder);
    // One file per run, not per route button pressed within it.
    let dir = pacenotes_installer::runlog::log_dir(&folder);
    if state.log.path().and_then(|path| path.parent()) == Some(dir.as_path()) {
        return;
    }
    let stamp =
        pacenotes_installer::utc_time::file_stamp(pacenotes_installer::utc_time::unix_now());
    state
        .log
        .attach(pacenotes_installer::runlog::log_path(&folder, &stamp));
    if let Some(path) = state.log.path() {
        crate::crash::log_to(path.to_path_buf());
    }
    if let Some(game) = state.game {
        let line = format!("game: {} in {}", game.title(), state.folder);
        super::progress_page::log_line(dialog, state, &line);
    }
}
