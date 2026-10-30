//! One dialog procedure per scene of `installer/DESIGN.md` rev. 2.

mod download;
mod download_tree_view;
mod game_folder;
mod language;
mod message_of_the_day;

pub use download::download_page;
pub use game_folder::game_folder_page;
pub use language::language_page;
pub use message_of_the_day::motd_page;
