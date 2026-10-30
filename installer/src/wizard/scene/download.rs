use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// Scene 4: the catalogue as a tree, checked by what the user wants this run.
pub unsafe extern "system" fn download_page(
    dialog: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match message {
        wm::WM_INITDIALOG => {
            crate::wizard::page::adopt_state(dialog, lparam);
            super::download_tree_view::enable_partial_checkboxes(dialog);
            1
        }
        crate::worker::WM_WORKER_DONE => {
            on_worker_done(dialog, wparam, lparam);
            1
        }
        wm::WM_NOTIFY => match crate::wizard::page::notify_code(lparam) {
            Some(ctl::PSN_SETACTIVE) => {
                crate::wizard::page::set_sheet_caption(dialog);
                crate::wizard::page::set_header(dialog, crate::i18n::IDS_S4_TITLE);
                crate::wizard::page::set_buttons(dialog, ctl::PSWIZB_BACK | ctl::PSWIZB_NEXT);
                crate::wizard::page::set_next_text(dialog, crate::i18n::IDS_BTN_DOWNLOAD);
                start_catalogue_fetch(dialog);
                crate::wizard::page::reply(dialog, 0)
            }
            Some(super::download_tree_view::TVN_ITEMCHANGEDW) => {
                super::download_tree_view::on_item_changed(dialog, lparam)
            }
            Some(ctl::PSN_WIZNEXT) => on_download(dialog),
            Some(ctl::PSN_QUERYCANCEL) => crate::wizard::page::query_cancel(dialog),
            _ => 0,
        },
        _ => 0,
    }
}

/// A failed fetch is not fatal: the tree falls back to what is already cached.
unsafe fn on_worker_done(dialog: HWND, wparam: WPARAM, lparam: LPARAM) {
    let Some(message) = crate::worker::take(lparam) else {
        return;
    };
    let Some(state) = crate::wizard::page::state(dialog) else {
        return;
    };
    if !state.is_current_request(wparam) {
        return;
    }
    if let crate::worker::Message::Packages(result) = message {
        let (packages, status) = match result {
            Ok(packages) => (packages, None),
            Err(_) => (Vec::new(), Some(crate::i18n::IDS_S4_FETCH_FAILED)),
        };
        build_tree(dialog, state, packages, status);
    }
}

/// Once per run; offline is a documented path that builds from the cache alone.
unsafe fn start_catalogue_fetch(dialog: HWND) {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return;
    };
    if state.catalogue_loaded {
        return;
    }
    state.catalogue_loaded = true;

    let Some(game) = state.game else {
        build_tree(dialog, state, Vec::new(), Some(crate::i18n::IDS_S4_EMPTY));
        return;
    };
    crate::ui::set_text(
        dialog,
        crate::wizard::control_ids::IDC_S4_STATUS,
        crate::i18n::IDS_S4_LOADING,
    );
    if state.online != Some(true) {
        build_tree(dialog, state, Vec::new(), Some(crate::i18n::IDS_S4_OFFLINE));
        return;
    }

    let request = state.next_request();
    let base_url = state.base_url.clone();
    let token = Some(state.secret.clone()).filter(|secret| !secret.is_empty());
    let game_id = game.id();
    crate::worker::spawn(dialog, request, move || {
        crate::worker::fetch_packages(base_url, token, game_id)
    });
}

/// Rebuilds the model from the catalogue plus the cache and shows it.
unsafe fn build_tree(
    dialog: HWND,
    state: &mut crate::wizard::state::WizardState,
    packages: Vec<pacenotes_shared::PackageInfo>,
    status: Option<u16>,
) {
    let game = state
        .game
        .map(|game| game.id())
        .unwrap_or(pacenotes_shared::GAME_RBR);
    // TODO(I5): read the cache index; until then nothing is cached.
    let cached: Vec<pacenotes_installer::tree::CachedPackage> = Vec::new();
    state.tree = pacenotes_installer::tree::Tree::build(game, &packages, &cached);

    super::download_tree_view::populate(dialog, state);

    let status = status.unwrap_or(if state.tree.is_empty() {
        crate::i18n::IDS_S4_EMPTY
    } else {
        crate::i18n::IDS_S4_READY
    });
    crate::ui::set_text(dialog, crate::wizard::control_ids::IDC_S4_STATUS, status);
}

/// Every category in the tree needs a pick; the transfer itself happens on the progress scene.
unsafe fn on_download(dialog: HWND) -> isize {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return crate::wizard::page::reply(dialog, 0);
    };
    let missing = state.tree.categories_missing_a_pick();
    if !missing.is_empty() {
        let text = crate::i18n::load_string_arg(crate::i18n::IDS_S4_MISSING, &missing.join(", "));
        crate::ui::message_box_warning(dialog, &text);
        return crate::wizard::page::stay(dialog);
    }
    state.chosen = state.tree.checked_keys();
    crate::wizard::page::reply(dialog, 0)
}
