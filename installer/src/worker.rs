//! The one background thread. A worker never touches a control: it posts
//! messages and the UI thread acts on them.

mod apply_selection;
mod channel;
mod download_run;
mod install_run;
mod message;
mod restore_run;
mod server_checks;

pub use channel::{spawn, spawn_reporting, take, take_progress, Reporter};
pub use download_run::{run_download, DownloadJob};
pub use install_run::{run_install, InstallJob};
pub use message::{
    Applied, Fetched, Message, NotDownloaded, Outcome, Phase, Progress, SecretOutcome,
    WM_WORKER_DONE, WM_WORKER_PROGRESS,
};
pub use restore_run::{run_restore, RestoreJob};
pub use server_checks::{check_health, check_secret, fetch_motd, fetch_packages};
