//! The one background thread. A worker never touches a control: it posts a
//! `Message` and the UI thread acts on it.

/// `WPARAM` is the request id, `LPARAM` a `Box<Message>` the receiver reclaims with `take`.
pub const WM_WORKER_DONE: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 1;

/// What a worker finished, one variant per thing a scene asks for.
pub enum Message {
    Health {
        online: bool,
    },
    /// `None` when the server has no text or cannot be reached.
    Motd(Option<String>),
    SecretChecked(SecretOutcome),
    /// A failure is not fatal: the Download scene falls back to the cache.
    Packages(Result<Vec<pacenotes_shared::PackageInfo>, String>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SecretOutcome {
    Accepted,
    /// 401: wrong or expired.
    Rejected,
    /// Could not ask, which DESIGN.md treats as permission to continue unvalidated.
    Unreachable,
}

/// Runs `work` on a new thread and posts its result to `window`, tagged with
/// `request` so a scene can drop an answer it has moved past.
pub fn spawn<F>(window: windows_sys::Win32::Foundation::HWND, request: usize, work: F)
where
    F: FnOnce() -> Message + Send + 'static,
{
    // A window handle is not `Send`, but `PostMessageW` may be called from any thread.
    let target = window as isize;
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

/// Reclaims the `Message` a `WM_WORKER_DONE` carries.
///
/// # Safety
/// Call exactly once per received `WM_WORKER_DONE`, with its `LPARAM`.
pub unsafe fn take(lparam: windows_sys::Win32::Foundation::LPARAM) -> Option<Message> {
    if lparam == 0 {
        return None;
    }
    Some(*Box::from_raw(lparam as *mut Message))
}

pub fn check_health(base_url: String) -> Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let online = crate::api::Api::new(&base_url, None).health(&cancel);
    Message::Health { online }
}

pub fn fetch_motd(base_url: String, lang: &'static str) -> Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    Message::Motd(crate::api::Api::new(&base_url, None).fetch_motd(lang, &cancel))
}

/// Asks for a listing with the secret; only a 401 stops the user, so a flaky
/// network never strands someone whose packages are already cached.
pub fn check_secret(base_url: String, secret: String) -> Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let api = crate::api::Api::new(&base_url, Some(secret));
    let any_game = pacenotes_shared::GAME_RBR;
    let outcome = match api.fetch_manifest(any_game, &cancel) {
        Ok(_) => SecretOutcome::Accepted,
        Err(e) if e.status == Some(401) => SecretOutcome::Rejected,
        Err(_) => SecretOutcome::Unreachable,
    };
    Message::SecretChecked(outcome)
}

pub fn fetch_packages(base_url: String, token: Option<String>, game: &'static str) -> Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    match crate::api::Api::new(&base_url, token).fetch_manifest(game, &cancel) {
        Ok(manifest) => Message::Packages(Ok(manifest.packages)),
        Err(e) => Message::Packages(Err(e.message)),
    }
}
