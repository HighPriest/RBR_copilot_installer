use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// Scene 2: the server's message of the day, and the optional access key.
pub unsafe extern "system" fn motd_page(
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
                crate::wizard::control_ids::IDC_S2_SECRET_LABEL,
                crate::i18n::IDS_S2_SECRET,
            );
            crate::ui::set_text(
                dialog,
                crate::wizard::control_ids::IDC_S2_STATUS,
                crate::i18n::IDS_S2_CHECKING,
            );
            // The offline copy shows until the server's text arrives, so the pane is never blank.
            crate::ui::set_text(
                dialog,
                crate::wizard::control_ids::IDC_S2_MOTD,
                crate::i18n::IDS_S2_OFFLINE_MOTD,
            );
            1
        }
        crate::worker::WM_WORKER_DONE => {
            on_worker_done(dialog, wparam, lparam);
            1
        }
        wm::WM_NOTIFY => match crate::wizard::page::notify_code(lparam) {
            Some(ctl::PSN_SETACTIVE) => {
                crate::wizard::page::set_sheet_caption(dialog);
                crate::wizard::page::set_header(dialog, crate::i18n::IDS_S2_TITLE);
                crate::wizard::page::set_buttons(dialog, ctl::PSWIZB_NEXT);
                crate::wizard::page::set_next_text(dialog, crate::i18n::IDS_BTN_ACCEPT);
                start_probes(dialog);
                crate::wizard::page::reply(dialog, 0)
            }
            Some(ctl::PSN_WIZNEXT) => on_accept(dialog),
            Some(ctl::PSN_QUERYCANCEL) => crate::wizard::page::query_cancel(dialog),
            _ => 0,
        },
        _ => 0,
    }
}

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
    match message {
        crate::worker::Message::Health { online } => {
            state.online = Some(online);
            let status = if online {
                crate::i18n::IDS_S2_ONLINE
            } else {
                crate::i18n::IDS_S2_OFFLINE
            };
            crate::ui::set_text(dialog, crate::wizard::control_ids::IDC_S2_STATUS, status);
        }
        crate::worker::Message::Motd(Some(text)) => {
            crate::ui::set_text_raw(dialog, crate::wizard::control_ids::IDC_S2_MOTD, &text);
        }
        crate::worker::Message::SecretChecked(outcome) => on_secret_checked(dialog, outcome),
        crate::worker::Message::Motd(None)
        | crate::worker::Message::Packages(_)
        | crate::worker::Message::Downloaded(_)
        | crate::worker::Message::Installed(_) => {}
    }
}

/// Asks once per run whether the server is up and what it has to say.
unsafe fn start_probes(dialog: HWND) {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return;
    };
    if state.online.is_some() {
        return;
    }
    let request = state.next_request();
    let base_url = state.base_url.clone();
    crate::worker::spawn(dialog, request, move || {
        crate::worker::check_health(base_url)
    });

    let base_url = state.base_url.clone();
    let lang = state.language.tag();
    crate::worker::spawn(dialog, request, move || {
        crate::worker::fetch_motd(base_url, lang)
    });
}

/// An empty key continues anonymously, and so does any key while offline;
/// otherwise the key is checked first and the page waits for the answer.
unsafe fn on_accept(dialog: HWND) -> isize {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return crate::wizard::page::reply(dialog, 0);
    };
    let typed = crate::ui::get_text(dialog, crate::wizard::control_ids::IDC_S2_SECRET)
        .trim()
        .to_string();
    if typed != state.secret {
        state.secret = typed;
        state.secret_checked = false;
    }
    if state.secret.is_empty() || state.secret_checked || state.online != Some(true) {
        return crate::wizard::page::reply(dialog, 0);
    }

    let request = state.next_request();
    let base_url = state.base_url.clone();
    let secret = state.secret.clone();
    crate::wizard::page::set_buttons(dialog, 0);
    crate::worker::spawn(dialog, request, move || {
        crate::worker::check_secret(base_url, secret)
    });
    crate::wizard::page::stay(dialog)
}

/// Short enough that most runs say nothing, long enough to ask for a new key
/// between two rally weekends.
const EXPIRY_NOTICE_DAYS: i64 = 14;

/// A rejected key keeps the user here; otherwise Next is pressed on their
/// behalf, after warning of a key about to lapse, which would otherwise look
/// like content being withdrawn. An unreachable server lets the key through unchecked.
unsafe fn on_secret_checked(dialog: HWND, outcome: crate::worker::SecretOutcome) {
    crate::wizard::page::set_buttons(dialog, ctl::PSWIZB_NEXT);
    match outcome {
        crate::worker::SecretOutcome::Rejected => {
            crate::ui::message_box_warning(
                dialog,
                &crate::i18n::load_string(crate::i18n::IDS_S2_BADSECRET),
            );
            return;
        }
        crate::worker::SecretOutcome::Accepted { expires_at } => {
            if let Some(date) = expiry_notice(expires_at, pacenotes_installer::engine::unix_now()) {
                crate::ui::message_box_info(
                    dialog,
                    &crate::i18n::load_string_arg(crate::i18n::IDS_S2_EXPIRING, &date),
                );
            }
        }
        crate::worker::SecretOutcome::Unreachable => {}
    }
    if let Some(state) = crate::wizard::page::state(dialog) {
        state.secret_checked = true;
    }
    wm::SendMessageW(
        crate::wizard::page::sheet(dialog),
        ctl::PSM_PRESSBUTTON,
        ctl::PSBTN_NEXT as WPARAM,
        0,
    );
}

/// The date a key lapses, or `None` while it is too far off to mention; a date
/// rather than a day count, which Polish would need three plural forms for.
fn expiry_notice(expires_at: Option<i64>, now: i64) -> Option<String> {
    let at = expires_at?;
    if at - now > EXPIRY_NOTICE_DAYS * 86_400 {
        return None;
    }
    let stamp = pacenotes_installer::engine::utc_stamp(at);
    Some(stamp.split('T').next().unwrap_or("").to_string())
}
