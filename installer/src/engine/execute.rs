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
pub use run::{execute, Outcome};
