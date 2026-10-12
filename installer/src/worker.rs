//! One background thread, talking to the UI only by posted messages.
//!
//! Task I4. installer/CLAUDE.md §4: no tokio — concurrency here is a single
//! `std::thread` plus `PostMessageW`, which is also what keeps the dialog
//! responsive while WinHTTP blocks.
//!
//! The rule the whole module exists to keep: **a worker thread never touches a
//! control.** It produces a `Message`, posts it, and the UI thread is the only
//! one that calls into user32. Anything else risks a cross-thread window
//! operation that works on a fast machine and deadlocks on a slow one.

#![allow(dead_code)]

/// Posted to the page that asked for the work. `WPARAM` is the request id so a
/// late answer to a superseded request can be discarded; `LPARAM` is a
/// `Box<Message>` leaked by the sender and reclaimed by the receiver.
pub const WM_WORKER_DONE: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 1;

/// What the worker finished. One variant per thing a scene asks for.
pub enum Message {
    /// Scene 2: is the server reachable at all (`GET /health`).
    Health { online: bool },
    /// Scene 2: the message of the day, or None when the server has no text
    /// for this language (or is unreachable — the scene shows its offline
    /// copy either way).
    Motd(Option<String>),
    /// Scene 2: result of validating a non-empty secret with `GET /packages`.
    SecretChecked(SecretOutcome),
    /// Scene 4: the catalogue for the detected game, or why it could not be
    /// had. A failure is not fatal — the scene falls back to the cache.
    Packages(Result<Vec<pacenotes_shared::PackageInfo>, String>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SecretOutcome {
    /// The server accepted it.
    Accepted,
    /// The server answered 401: the secret is wrong or expired.
    Rejected,
    /// Could not ask. DESIGN.md: offline means continue with the secret
    /// unvalidated, so this is not a refusal.
    Unreachable,
}

/// Run `work` on a new thread and post its result to `hwnd`.
///
/// `request` is echoed back as WPARAM. A scene bumps it whenever it starts a
/// new request so that a slow earlier answer — a 10 s connect timeout landing
/// after the user already moved on — can be recognised and dropped.
pub fn spawn<F>(hwnd: windows_sys::Win32::Foundation::HWND, request: usize, work: F)
where
    F: FnOnce() -> Message + Send + 'static,
{
    // HWND is a raw pointer and so not Send; it is however just a handle, and
    // PostMessageW is explicitly safe to call from any thread. Carry it across
    // as an integer to keep that reasoning visible rather than silencing the
    // compiler with an unsafe impl.
    let target = hwnd as isize;
    std::thread::spawn(move || {
        let message = Box::new(work());
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                target as windows_sys::Win32::Foundation::HWND,
                WM_WORKER_DONE,
                request,
                Box::into_raw(message) as windows_sys::Win32::Foundation::LPARAM,
            );
        }
    });
}

/// Reclaim the `Message` a `WM_WORKER_DONE` carries.
///
/// # Safety
/// Must be called exactly once per received `WM_WORKER_DONE`, with that
/// message's LPARAM. Skipping it leaks the box; calling it twice frees twice.
pub unsafe fn take(lparam: windows_sys::Win32::Foundation::LPARAM) -> Option<Message> {
    if lparam == 0 {
        return None;
    }
    Some(*Box::from_raw(lparam as *mut Message))
}

// ---- The jobs themselves. Pure "do a request, report what happened". ----

/// `GET /health`.
pub fn check_health(base_url: String) -> Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let online = crate::api::Api::new(&base_url, None).health(&cancel);
    Message::Health { online }
}

/// `GET /motd?lang=xx`.
///
/// A 404 is the normal answer when the operator has published no text, so it
/// is reported the same as being offline: the scene shows its built-in copy.
pub fn fetch_motd(base_url: String, lang: &'static str) -> Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    Message::Motd(crate::api::Api::new(&base_url, None).fetch_motd(lang, &cancel))
}

/// Validate a secret by asking for the listing it is supposed to widen.
///
/// DESIGN.md scene 2: 401 is the only outcome that stops the user. Any other
/// failure is treated as "could not ask", because refusing to proceed when the
/// network is flaky would strand someone who has everything cached already.
pub fn check_secret(base_url: String, secret: String) -> Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let api = crate::api::Api::new(&base_url, Some(secret));
    // Any game will do; this asks whether the credential is accepted at all.
    let outcome = match api.fetch_manifest(pacenotes_shared::GAME_RBR, &cancel) {
        Ok(_) => SecretOutcome::Accepted,
        Err(e) if e.status == Some(401) => SecretOutcome::Rejected,
        Err(_) => SecretOutcome::Unreachable,
    };
    Message::SecretChecked(outcome)
}

/// `GET /packages?game=<id>` for the game scene 3 detected.
pub fn fetch_packages(base_url: String, token: Option<String>, game: &'static str) -> Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    match crate::api::Api::new(&base_url, token).fetch_manifest(game, &cancel) {
        Ok(manifest) => Message::Packages(Ok(manifest.packages)),
        Err(e) => Message::Packages(Err(e.message)),
    }
}
