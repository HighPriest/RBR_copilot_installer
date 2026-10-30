/// One installable package as listed by `GET /packages`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PackageInfo {
    /// The access directory, `[a-z0-9_-]+`.
    pub directory: String,
    /// The path under `directory` without `.zip`, e.g. `codriver/polish/male/Kamil`.
    /// A lookup key: check it with `valid_name`, never join it onto a filesystem path.
    pub name: String,
    pub version: String,
    /// Bytes in the zip.
    pub size: u64,
    /// Lowercase hex sha256 of the zip, which the installer verifies after downloading.
    pub sha256: String,
    /// Unix seconds.
    pub updated_at: i64,
    /// One of `GAME_*`.
    #[serde(default = "crate::default_game")]
    pub game: String,
    /// Renames the leaf only; the levels above it always come from `name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Packages in one group overwrite the same files, so at most one of them is installed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusive_group: Option<String>,
    /// `PackageRef`s in their `directory/name` text form.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
}

impl PackageInfo {
    pub fn is_public(&self) -> bool {
        self.directory == crate::PUBLIC_DIR
    }

    pub fn package_ref(&self) -> crate::PackageRef {
        crate::PackageRef {
            directory: self.directory.clone(),
            name: self.name.clone(),
        }
    }

    pub fn segments(&self) -> std::str::Split<'_, char> {
        self.name.split('/')
    }

    /// The first segment; a name without a `/` is its own category.
    pub fn category(&self) -> &str {
        self.name.split('/').next().unwrap_or(&self.name)
    }

    /// The segments between the category and the leaf; empty for `style/Descriptive`.
    pub fn tree_levels(&self) -> Vec<&str> {
        let segments: Vec<&str> = self.segments().collect();
        match segments.len() {
            0..=2 => Vec::new(),
            count => segments[1..count - 1].to_vec(),
        }
    }

    pub fn leaf(&self) -> &str {
        self.name.rsplit('/').next().unwrap_or(&self.name)
    }

    /// `display_name` when set, otherwise the leaf, never the whole path.
    pub fn label(&self) -> &str {
        self.display_name.as_deref().unwrap_or_else(|| self.leaf())
    }

    /// The tail of `GET /download/<game>/<directory>/<name>`.
    pub fn download_path(&self) -> String {
        format!("{}/{}/{}", self.game, self.directory, self.name)
    }

    /// Drops malformed entries, which `manifest gen` already warned the publisher about.
    pub fn required_refs(&self) -> Vec<crate::PackageRef> {
        self.requires
            .iter()
            .filter_map(|text| crate::PackageRef::parse(text))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_name_is_the_packages_place_in_the_tree() {
        let package = crate::fixtures::package(crate::PUBLIC_DIR, "codriver/polish/male/Kamil");
        assert_eq!(package.category(), crate::CATEGORY_CODRIVER);
        assert_eq!(package.tree_levels(), vec!["polish", "male"]);
        assert_eq!(package.leaf(), "Kamil");
        assert_eq!(package.label(), "Kamil");

        let package = crate::fixtures::package(crate::PUBLIC_DIR, "style/Descriptive");
        assert_eq!(package.category(), crate::CATEGORY_STYLE);
        assert!(package.tree_levels().is_empty());
        assert_eq!(package.leaf(), "Descriptive");

        let package = crate::fixtures::package(crate::PUBLIC_DIR, "basic");
        assert_eq!(package.category(), "basic");
        assert!(package.tree_levels().is_empty());
        assert_eq!(package.leaf(), "basic");
    }

    #[test]
    fn round_trip_keeps_install_metadata() {
        let mut package = crate::fixtures::package("premium-voices", "codriver/polish/male/Kamil");
        package.game = crate::GAME_DIRT_RALLY2.into();
        package.display_name = Some("Kamil Filipek".into());
        package.exclusive_group = Some("pacenote-config".into());
        package.requires = vec!["public/language/english".into()];

        let json = serde_json::to_string(&package).unwrap();
        let back: super::PackageInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.game, crate::GAME_DIRT_RALLY2);
        assert_eq!(back.label(), "Kamil Filipek");
        assert_eq!(back.exclusive_group.as_deref(), Some("pacenote-config"));
        assert_eq!(
            back.required_refs(),
            vec![crate::PackageRef {
                directory: "public".into(),
                name: "language/english".into()
            }]
        );
    }

    /// Retired fields are ignored rather than rejected, because a newer installer reads an older cache.
    #[test]
    fn an_older_package_entry_still_deserializes() {
        let old = r#"{"directory":"public","name":"basic",
            "version":"1.0.0","size":5,"sha256":"ab","updated_at":1,
            "category":"pacenotes","tree_path":["ignored"]}"#;
        let package: super::PackageInfo = serde_json::from_str(old).unwrap();
        assert_eq!(package.game, crate::GAME_RBR);
        assert_eq!(package.label(), "basic");
        assert!(package.display_name.is_none());
        assert!(package.exclusive_group.is_none());
        assert!(package.requires.is_empty());
        assert_eq!(package.category(), "basic");
        assert!(package.tree_levels().is_empty());
    }

    #[test]
    fn empty_metadata_is_not_serialized() {
        let json =
            serde_json::to_string(&crate::fixtures::package(crate::PUBLIC_DIR, "basic")).unwrap();
        assert!(!json.contains("display_name"), "{json}");
        assert!(!json.contains("exclusive_group"), "{json}");
        assert!(!json.contains("requires"), "{json}");
        assert!(json.contains("\"game\":\"rbr\""), "{json}");
        assert!(!json.contains("category"), "{json}");
        assert!(!json.contains("tree_path"), "{json}");
    }

    #[test]
    fn malformed_requires_are_dropped_not_fatal() {
        let mut package = crate::fixtures::package(crate::PUBLIC_DIR, "voice");
        package.requires = vec!["public/lang".into(), "garbage".into()];
        assert_eq!(package.required_refs().len(), 1);
    }

    #[test]
    fn download_path_includes_the_game() {
        let mut package = crate::fixtures::package(crate::PUBLIC_DIR, "codriver/polish/male/Kamil");
        assert_eq!(
            package.download_path(),
            "rbr/public/codriver/polish/male/Kamil"
        );
        package.game = crate::GAME_BEAMNG.into();
        assert_eq!(
            package.download_path(),
            "beamng/public/codriver/polish/male/Kamil"
        );
    }
}
