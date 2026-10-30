//! The wizard: a comctl32 PropertySheet in wizard mode, one page per scene of
//! `installer/DESIGN.md` rev. 2. Scene 1 shares the sheet's frame rather than
//! being the small window DESIGN.md describes, since a sheet has one size.

mod control_ids;
mod page;
mod scene;
mod sheet;
mod state;

pub use sheet::run;
