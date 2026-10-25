/// A game's schema file on the server, `<data>/<game>/schema.toml`; the
/// canonical copy is the repository's `schemas/<game>.toml`.
pub const SCHEMA_FILE: &str = "schema.toml";

/// `SchemaCategory::select`: the user must pick exactly one.
pub const SELECT_ONE: &str = "one";
/// `SchemaCategory::select`: optional, installed as ticked.
pub const SELECT_ANY: &str = "any";

/// `SchemaCategory::install_root`: the folder holding the game's marker executable.
pub const INSTALL_ROOT_GAME: &str = "game";

/// A game's install schema as the manifest publishes it: which categories its
/// catalogue has, which ask for one pick, and where each installs.
///
/// The installer only compares `sha256` with the schema it was built with and
/// stops downloading on a mismatch; its behaviour never comes from these
/// fields, so a server cannot steer where files go. Every field is a plain
/// `String` so a newer server's values fail an old installer's comparison, never its parse.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GameSchema {
    /// Of the schema file's exact bytes, lowercase hex.
    pub sha256: String,
    pub categories: Vec<SchemaCategory>,
}

/// One `[[category]]` of a game's schema.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SchemaCategory {
    /// The folder under an access directory, as `PackageInfo::category()` reports it.
    pub id: String,
    /// `SELECT_ONE` or `SELECT_ANY`.
    pub select: String,
    /// A token such as `INSTALL_ROOT_GAME`, never a path.
    pub install_root: String,
}

#[cfg(test)]
mod tests {
    #[test]
    fn unknown_schema_values_never_break_parsing() {
        let future = r#"{"sha256":"cc","categories":[
            {"id":"x","select":"some-of","install_root":"user-docs"}]}"#;
        let schema: super::GameSchema = serde_json::from_str(future).unwrap();
        assert_eq!(schema.categories[0].select, "some-of");
    }
}
