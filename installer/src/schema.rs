//! The install schemas this build embeds, compiled from `schemas/<game>.toml`
//! by `build.rs`, and the one comparison made against the server's.
//!
//! A game's schema says which categories its catalogue has and which ask for a
//! single pick; the installer takes that from its own copy, never the server's.
//! The manifest's per-game hash of the schema's raw bytes is only compared: an
//! unknown one stops DOWNLOADING that game, and nothing else.

pub struct Schema {
    pub game: &'static str,
    /// The file's exact bytes, which the hash covers.
    pub toml: &'static [u8],
    pub categories: &'static [Category],
}

pub struct Category {
    pub id: &'static str,
    /// `pacenotes_shared::SELECT_ONE` or `SELECT_ANY`.
    pub select: &'static str,
    /// A token such as `pacenotes_shared::INSTALL_ROOT_GAME`, never a path.
    pub install_root: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/schema_table.rs"));

/// Older revisions this build still accepts, per game, each with a line saying
/// why the change does not matter; empty while `schemas/rbr.toml` has one revision.
const ALSO_ACCEPTED: &[(&str, &[&str])] = &[];

pub fn of(game: &str) -> Option<&'static Schema> {
    SCHEMAS.iter().find(|schema| schema.game == game)
}

/// Of the schema file this build was compiled with.
pub fn sha256_of(game: &str) -> Option<String> {
    of(game).map(|schema| crate::sha256::hex_digest(schema.toml))
}

/// The schema we carry, or an older revision deemed compatible.
pub fn accepts(game: &str, sha256: &str) -> bool {
    if sha256_of(game).is_some_and(|ours| ours == sha256) {
        return true;
    }
    ALSO_ACCEPTED
        .iter()
        .any(|(id, hashes)| *id == game && hashes.contains(&sha256))
}

/// The questions scene 5 asks and scene 4 demands a tick for, in schema order;
/// everything else is optional, so an operator's new folder adds no question.
pub fn choice_categories(game: &str) -> Vec<&'static str> {
    categories(game)
        .iter()
        .filter(|category| category.select == pacenotes_shared::SELECT_ONE)
        .map(|category| category.id)
        .collect()
}

/// In file order, which is the order the tree shows them in.
pub fn categories(game: &str) -> &'static [Category] {
    of(game).map(|schema| schema.categories).unwrap_or(&[])
}

/// Whether every category installs somewhere this build knows how to place.
pub fn install_roots_known(game: &str) -> bool {
    categories(game)
        .iter()
        .all(|category| category.install_root == pacenotes_shared::INSTALL_ROOT_GAME)
}

/// Why a game's catalogue cannot be downloaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Incompatible {
    /// The server predates schemas, or publishes nothing for the game.
    NoSchemaPublished,
    GameUnknown,
    Different {
        ours: String,
        theirs: String,
        /// Declared by the server and unknown here, usually the whole explanation.
        unknown_categories: Vec<String>,
    },
}

/// An error blocks downloads for this game only; the caller leaves everything else alone.
pub fn check(
    game: &str,
    published: Option<&pacenotes_shared::GameSchema>,
) -> Result<(), Incompatible> {
    let Some(published) = published else {
        return Err(Incompatible::NoSchemaPublished);
    };
    if of(game).is_none() {
        return Err(Incompatible::GameUnknown);
    }
    if accepts(game, &published.sha256) {
        return Ok(());
    }
    let known: Vec<&str> = categories(game)
        .iter()
        .map(|category| category.id)
        .collect();
    Err(Incompatible::Different {
        ours: sha256_of(game).unwrap_or_default(),
        theirs: published.sha256.clone(),
        unknown_categories: published
            .categories
            .iter()
            .filter(|category| !known.contains(&category.id.as_str()))
            .map(|category| category.id.clone())
            .collect(),
    })
}

#[cfg(test)]
mod tests;
