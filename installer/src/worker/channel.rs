/// Runs `work` on a new thread and posts its result to `window`, tagged with
/// `request` so a scene can drop an answer it has moved past.
pub fn spawn<F>(window: windows_sys::Win32::Foundation::HWND, request: usize, work: F)
where
    F: FnOnce() -> super::Message + Send + 'static,
{
    spawn_reporting(window, request, move |_| work());
}

/// Like `spawn`, for a job that posts progress through the `Reporter` it is
/// handed instead of a window handle it could misuse.
pub fn spawn_reporting<F>(window: windows_sys::Win32::Foundation::HWND, request: usize, work: F)
where
    F: FnOnce(&Reporter) -> super::Message + Send + 'static,
{
    // A window handle is not `Send`, but `PostMessageW` may be called from any thread.
    let target = window as isize;
    std::thread::spawn(move || {
        let reporter = Reporter { target, request };
        let message = work(&reporter);
        post(target, super::WM_WORKER_DONE, request, message);
    });
}

/// A job's one-way channel to the UI thread.
pub struct Reporter {
    target: isize,
    request: usize,
}

impl Reporter {
    pub fn send(&self, progress: super::Progress) {
        post(
            self.target,
            super::WM_WORKER_PROGRESS,
            self.request,
            progress,
        );
    }
}

fn post<T>(target: isize, message_id: u32, request: usize, payload: T) {
    let boxed = Box::new(payload);
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
            target as windows_sys::Win32::Foundation::HWND,
            message_id,
            request,
            Box::into_raw(boxed) as windows_sys::Win32::Foundation::LPARAM,
        );
    }
}

/// Reclaims the `Message` a `WM_WORKER_DONE` carries.
///
/// # Safety
/// Call exactly once per received `WM_WORKER_DONE`, with its `LPARAM`.
pub unsafe fn take(lparam: windows_sys::Win32::Foundation::LPARAM) -> Option<super::Message> {
    if lparam == 0 {
        return None;
    }
    Some(*Box::from_raw(lparam as *mut super::Message))
}

/// Reclaims the `Progress` a `WM_WORKER_PROGRESS` carries.
///
/// # Safety
/// Call exactly once per received `WM_WORKER_PROGRESS`, with its `LPARAM`.
pub unsafe fn take_progress(
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> Option<Box<super::Progress>> {
    if lparam == 0 {
        return None;
    }
    Some(Box::from_raw(lparam as *mut super::Progress))
}
