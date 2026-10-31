//! Carrying out a plan. The order is fixed: open a restore point holding the
//! ledger as it is now, run the uninstalls, then per package copy out what it
//! replaces, write its ledger entry as `installing`, and extract.

mod execute_error;
mod game_files;
mod install_package;
mod record_configured;
mod restore_point;
mod run;
mod uninstall_package;

pub use execute_error::ExecuteError;
pub use record_configured::record_configured;
pub use restore_point::{snapshot_path, RESTORE_DIR};
pub use run::{execute, Outcome};

pub(crate) use install_package::{install_package, Installing, Progress};
pub(crate) use restore_point::open_restore_point;
pub(crate) use run::RunContext;
pub(crate) use uninstall_package::uninstall_package;
