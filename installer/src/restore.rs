//! Taking a game folder back to a restore point, for a user handed a bad
//! package who wants yesterday back: whatever they had configured then, and
//! none of the files the bad package added. See `DESIGN.md` "Restore scene".
//!
//! Every run writes a point holding the ledger as it was before it, plus the
//! files it replaced; this module reads them back, plans the way to one and
//! carries that plan out through the engine's own steps. Three rules:
//!
//! 1. **The plan is a value, computed before anything moves**, so the preview
//!    the user approves and the work done cannot disagree.
//! 2. **Never make it worse.** A package that cannot be put back is left as it
//!    is, and nothing is removed unless what replaces it is ready.
//! 3. **Restore the provenance, not just the bytes.** The snapshot's per-file
//!    `prior` is what stops a later uninstall deleting a file the user always had.

mod changes;
mod execute;
mod plan;
mod point;

pub use changes::{changes, Changes};
pub use execute::{execute, prepare, Prepared};
pub use plan::{ledger_after, picks, plan, Plan, PutBack};
pub use point::{human_time, payloads_in_use, points, read, Point, Snapshot};

#[cfg(test)]
mod tests;
