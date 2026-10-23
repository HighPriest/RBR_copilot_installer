/// `WPARAM` is the request id, `LPARAM` a `Box<Message>` the receiver reclaims with `take`.
pub const WM_WORKER_DONE: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 1;

/// Posted while a job runs: `WPARAM` the request id, `LPARAM` a `Box<Progress>` for `take_progress`.
pub const WM_WORKER_PROGRESS: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 2;

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
    Installed(Outcome),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SecretOutcome {
    Accepted,
    /// 401: wrong or expired.
    Rejected,
    /// Could not ask, which DESIGN.md treats as permission to continue unvalidated.
    Unreachable,
}

/// How scene 6's run ended, which scene 7 reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Done {
        downloaded: usize,
        bytes: u64,
    },
    /// Already readable, and already in the log.
    Failed(String),
    /// Nothing installed, and the partial download discarded.
    Cancelled,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Download,
    Install,
}

/// Everything scene 6 shows, so the scene keeps no state beyond the last post.
pub struct Progress {
    pub phase: Phase,
    /// Already labelled for display.
    pub item: String,
    pub item_done: u64,
    pub item_size: u64,
    /// Across the whole phase, so the bar measures the run rather than restarting per package.
    pub done: u64,
    pub total: u64,
    /// A line for the log pane; most posts carry none.
    pub log: Option<String>,
}
