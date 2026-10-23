//! The one background thread. A worker never touches a control: it posts
//! messages and the UI thread acts on them.

mod channel;
mod install_run;
mod message;
mod server_checks;

pub use channel::{spawn, spawn_reporting, take, take_progress, Reporter};
pub use install_run::run_install;
pub use message::{
    Message, Outcome, Phase, Progress, SecretOutcome, WM_WORKER_DONE, WM_WORKER_PROGRESS,
};
pub use server_checks::{check_health, check_secret, fetch_motd, fetch_packages};
