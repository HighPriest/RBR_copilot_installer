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
        super::download_tree_view::WM_SYNC_CHECKS => {
            if let Some(state) = crate::wizard::page::state(dialog) {
                super::download_tree_view::refresh_checks(dialog, state);
            }
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
        match result {
            Ok(manifest) => {
                if state.online == Some(false) {
                    state
                        .log
                        .line("the catalogue answered although /health had not");
                }
                state.schema_mismatch = schema_mismatch(state, &manifest);
                build_tree(dialog, state, manifest.packages, None);
            }
            Err(reason) => {
                // The only diagnostic a user can send back, though the scene recovers.
                state
                    .log
                    .line(format!("the catalogue could not be read: {reason}"));
                // Two failures in a row read as the server being down, one as the catalogue failing.
                let status = if state.online == Some(false) {
                    crate::i18n::IDS_S4_OFFLINE
                } else {
                    crate::i18n::IDS_S4_FETCH_FAILED
                };
                build_tree(dialog, state, Vec::new(), Some(status));
            }
        }
    }
}

/// Checked only on a manifest the server sent, so an offline run is never
/// told it is out of date; the reason goes to the run log.
fn schema_mismatch(
    state: &mut crate::wizard::state::WizardState,
    manifest: &pacenotes_shared::Manifest,
) -> bool {
    let game = state.game_id();
    match pacenotes_installer::schema::check(game, manifest.schemas.get(game)) {
        Ok(()) => false,
        Err(reason) => {
            state.log.line(describe_schema_mismatch(game, &reason));
            true
        }
    }
}

/// The modal is short; this is what the user can send back.
fn describe_schema_mismatch(
    game: &str,
    reason: &pacenotes_installer::schema::Incompatible,
) -> String {
    match reason {
        pacenotes_installer::schema::Incompatible::NoSchemaPublished => format!(
            "the server published no install schema for {game}, so this installer cannot \
             tell whether it understands the catalogue"
        ),
        pacenotes_installer::schema::Incompatible::GameUnknown => {
            format!("this installer has no install schema for {game}")
        }
        pacenotes_installer::schema::Incompatible::Different {
            ours,
            theirs,
            unknown_categories,
        } => {
            let detail = if unknown_categories.is_empty() {
                "no category is new, so this is probably only an edit to the schema file"
                    .to_string()
            } else {
                format!(
                    "it has categories this installer does not know: {}",
                    unknown_categories.join(", ")
                )
            };
            format!(
                "the {game} catalogue uses a different install schema ({detail}). \
                 Server {theirs}, this installer {ours}"
            )
        }
    }
}

/// Once per run, and whatever `/health` said: it is a convenience for scene 2's
/// status line, and a hiccup there, or an answer still on its way, must not
/// hide a catalogue that would have come back (D12). A failed fetch builds the
/// tree from the cache alone.
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
    let request = state.next_request();
    let base_url = state.base_url.clone();
    let token = state.token();
    let game_id = game.id();
    crate::worker::spawn(dialog, request, move || {
        crate::worker::fetch_packages(base_url, token, game_id)
    });
}

/// Rebuilds the model from the catalogue, the cache and the game folder's
/// ledger, and shows it.
unsafe fn build_tree(
    dialog: HWND,
    state: &mut crate::wizard::state::WizardState,
    packages: Vec<pacenotes_shared::PackageInfo>,
    status: Option<u16>,
) {
    load_tree(state, packages);
    super::download_tree_view::populate(dialog, state);

    let status = status.unwrap_or(if state.tree.is_empty() {
        crate::i18n::IDS_S4_EMPTY
    } else {
        crate::i18n::IDS_S4_READY
    });
    crate::ui::set_text(dialog, crate::wizard::control_ids::IDC_S4_STATUS, status);
}

/// The model alone, which the Configure route needs for scene 5 though it never
/// shows scene 4's tree control.
pub(super) fn load_tree(
    state: &mut crate::wizard::state::WizardState,
    packages: Vec<pacenotes_shared::PackageInfo>,
) {
    let game = state.game_id();
    let cached = state.cache.entries();
    let installed: Vec<String> = pacenotes_installer::ledger::Ledger::open(&state.folder)
        .packages()
        .keys()
        .cloned()
        .collect();
    state.tree = pacenotes_installer::tree::Tree::build(game, &packages, &cached, &installed);
    state.packages = packages;
    // Sized with the model, all zero: a stray notification indexing an empty table aborted the Configure route.
    state.tree_items = vec![0; state.tree.len()];
}

/// Every choice category in the tree needs a pick, and a build too old for the
/// catalogue stops only when something must come down the wire, so it can still
/// reconfigure and remove; scene 4b does the transfers.
unsafe fn on_download(dialog: HWND) -> isize {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return crate::wizard::page::reply(dialog, 0);
    };
    let choices = pacenotes_installer::schema::choice_categories(state.game_id());
    let missing = state.tree.categories_missing_a_pick(&choices);
    if !missing.is_empty() {
        let text = crate::i18n::load_string_arg(crate::i18n::IDS_S4_MISSING, &missing.join(", "));
        crate::ui::message_box_warning(dialog, &text);
        return crate::wizard::page::stay(dialog);
    }
    if state.schema_mismatch && !state.tree.checked_needing_download().is_empty() {
        crate::ui::message_box_warning(
            dialog,
            &crate::i18n::load_string(crate::i18n::IDS_S4_NEEDS_NEWER),
        );
        return crate::wizard::page::stay(dialog);
    }
    state.chosen = state.tree.checked_keys();
    crate::wizard::page::reply(dialog, 0)
}
