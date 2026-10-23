/// What the cache holds for one package, as the tree needs to see it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CachedPackage {
    pub game: String,
    pub directory: String,
    pub name: String,
    pub sha256: String,
    pub version: String,
    pub display_name: Option<String>,
}

impl CachedPackage {
    /// `game/directory/name`, the same string as `PackageInfo::download_path()`;
    /// without the game, two games' `public/style/Numeric Standard` would collide.
    pub fn key(&self) -> String {
        format!("{}/{}/{}", self.game, self.directory, self.name)
    }
}
