//! The apply engine: plan from selection + ledger + cache, then execute.
//!
//! `plan` is a pure function over values the caller has already read, so the
//! decision table is testable anywhere and a plan is reproducible.
//!
//! TODO(I7d): `execute()`, restore points under `.pacenotes\restore\<time>\`,
//! and putting files back.

mod overlap;
mod plan;

pub use overlap::{overlaps, Overlap};
pub use plan::{plan, Action, Plan, PlanError, PlanInput};
