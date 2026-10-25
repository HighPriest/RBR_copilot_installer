//! Protocol types shared between the installer and the server.
//!
//! Serde types only: no I/O, no async, no HTTP. Timestamps are unix seconds.
//! Any change here is a protocol change and lands on both sides in one PR.
//!
//! A package's address is `<data>/<game>/packages/<directory>/<name>.zip`.
//! `directory` decides who may see it (`PUBLIC_DIR` is everyone's), and
//! `name` is its path under that directory, which is also its place in the
//! installer's tree: `codriver/polish/male/Kamil` is category `codriver`,
//! levels `polish` → `male`, leaf `Kamil`. Both stay lookup keys that are
//! never joined onto a filesystem path.

mod access;
mod api_error;
mod category;
mod download_path;
mod game;
mod lookup_key;
mod manifest;
mod package;
mod package_ref;

#[cfg(test)]
mod fixtures;

pub use access::{AUTH_HEADER, BEARER_PREFIX, PUBLIC_DIR};
pub use api_error::ApiError;
pub use category::{
    CATEGORY_CODRIVER, CATEGORY_LANGUAGE, CATEGORY_PACENOTES, CATEGORY_STYLE, CHOICE_CATEGORIES,
    KNOWN_CATEGORIES,
};
pub use download_path::{category_of_download_path, split_download_path};
pub use game::{
    default_game, GAME_ASSETTOCORSA, GAME_BEAMNG, GAME_DIRT_RALLY2, GAME_RBR, KNOWN_GAMES,
};
pub use lookup_key::{valid_id, valid_name};
pub use manifest::Manifest;
pub use package::PackageInfo;
pub use package_ref::PackageRef;
