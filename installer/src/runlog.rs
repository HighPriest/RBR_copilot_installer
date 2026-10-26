//! The run log (DESIGN.md "Logging"): in memory for the progress panes, and on
//! disk line by line for after a run that died. Only the UI thread writes it;
//! workers post their lines.

mod log_files;
mod run_log;

pub use log_files::{log_dir, log_path, prune, KEEP_LOGS};
pub use run_log::Log;

#[cfg(test)]
mod tests;
