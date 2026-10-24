/// What the cache holds for one package, as the tree needs to see it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CachedPackage {
    pub game: String,
    pub directory: String,
    pub name: String,
    pub sha256: String,
    pub version: String,
    pub display_name: Option<String>,
    /// The rest of the manifest entry, the only record an offline run has; each
    /// defaults so an index from before they existed still loads.
    #[serde(default)]
    pub exclusive_group: Option<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub updated_at: i64,
}

impl CachedPackage {
    /// The snapshot taken when a package enters the cache.
    pub fn of(package: &pacenotes_shared::PackageInfo) -> CachedPackage {
        CachedPackage {
            game: package.game.clone(),
            directory: package.directory.clone(),
            name: package.name.clone(),
            sha256: package.sha256.clone(),
            version: package.version.clone(),
            display_name: package.display_name.clone(),
            exclusive_group: package.exclusive_group.clone(),
            requires: package.requires.clone(),
            size: package.size,
            updated_at: package.updated_at,
        }
    }

    /// `game/directory/name`, the same string as `PackageInfo::download_path()`;
    /// without the game, two games' `public/style/Numeric Standard` would collide.
    pub fn key(&self) -> String {
        format!("{}/{}/{}", self.game, self.directory, self.name)
    }
}
