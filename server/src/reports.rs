//! Conflict reports from installers: two selected packages wrote the same
//! file, the later one won, and only the operator can fix the catalogue.
//!
//! An unauthenticated write, so the body is capped, every field validated,
//! nothing echoed, the rate limited per client and overall, and storage is
//! append-only lines. Nothing stored identifies a user: the client address
//! the limiter needs stays in memory.

mod cidr;
mod client_address;
mod overlap_summary;
mod rate_limiter;
mod report_file;
mod report_log;
mod validation;

pub use cidr::Cidr;
pub use client_address::client_ip;
pub use overlap_summary::{overlap_warnings, SUMMARY_DAYS};
pub use rate_limiter::{Limits, RateLimiter};
pub use report_file::{file_name, utc_date};
pub use report_log::{Record, Reports};
pub use validation::{
    validate, Validated, MAX_BODY_BYTES, MAX_CONFLICTS, MAX_FILE_BYTES, MAX_INSTALLER_LEN,
    MAX_PACKAGES, MIN_PACKAGES,
};
