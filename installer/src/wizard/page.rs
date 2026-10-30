//! What every wizard page does the same way: find the shared state, dress the
//! sheet for its scene, and answer the sheet's notifications.

mod chrome;
mod notification;
mod state_access;

pub use chrome::{set_buttons, set_header, set_next_text, set_sheet_caption, sheet};
pub use notification::{notify_code, query_cancel, reply, stay};
pub use state_access::{adopt_state, state};
