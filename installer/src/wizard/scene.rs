//! One dialog procedure per scene of `installer/DESIGN.md` rev. 2.

mod configuration;
mod download;
mod download_tree_view;
mod fetching;
mod game_folder;
mod game_folder_routes;
mod installation;
mod language;
mod message_of_the_day;
mod progress_page;
mod restore;
mod summary;

pub use configuration::configuration_page;
pub use download::download_page;
pub use fetching::fetching_page;
pub use game_folder::game_folder_page;
pub use installation::installation_page;
pub use language::language_page;
pub use message_of_the_day::motd_page;
pub use restore::restore_page;
pub use summary::summary_page;
