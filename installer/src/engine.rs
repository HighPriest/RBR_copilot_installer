//! The apply engine: plan from selection + ledger + cache, check, then execute.
//!
//! `plan` is a pure function over values the caller has already read, so the
//! decision table is testable anywhere and a plan is reproducible.

mod execute;
mod overlap;
mod plan;
mod preflight;
mod watcher;

pub use execute::{execute, ExecuteError, Outcome};
pub use overlap::{overlaps, Overlap};
pub use plan::{plan, Action, Plan, PlanError, PlanInput};
pub use preflight::{preflight, HostFacts, Preflight, PreflightError};
pub use watcher::{Phase, Silent, Watcher};
