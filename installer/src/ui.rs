//! The Win32 vocabulary the wizard pages are written in.

mod combo_box;
mod control_layout;
mod control_text;
mod explorer;
mod folder_picker;
mod forward_button;
mod list_box;
mod message_box;
mod resource;

pub use combo_box::{combo_add, combo_reset, combo_select, combo_selection};
pub use control_layout::{scroll_to_end, show, stretch_to_bottom_of};
pub use control_text::{append_line, enable, get_text, set_text, set_text_raw, set_window_text};
pub use explorer::reveal_in_explorer;
pub use folder_picker::browse_for_folder;
pub use forward_button::paint_forward_button;
pub use list_box::{list_add, list_reset, list_select, list_selection, LBN_SELCHANGE};
pub use message_box::{
    ask_error, confirm, message_box_error, message_box_info, message_box_warning,
};
pub use resource::{hiword, load_app_icon, loword, make_int_resource};

pub const IDCANCEL: u16 = 2;
