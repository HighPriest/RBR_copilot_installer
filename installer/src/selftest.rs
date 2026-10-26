//! Developer checks that run one real network path without the wizard and
//! report to a file or a MessageBox, since a GUI-subsystem binary has no stdout.

mod conflict_report_check;
mod download_check;
mod http_check;
mod panic_check;
mod report;

pub use conflict_report_check::ConflictReportSelfTest;
pub use download_check::DownloadSelfTest;
pub use http_check::HttpSelfTest;
pub use panic_check::panic_if_requested;
