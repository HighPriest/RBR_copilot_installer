/// The body of `GET /packages`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct Manifest {
    pub packages: Vec<crate::PackageInfo>,
}

impl Manifest {
    /// Keeps `public` plus the directories named in `grants`.
    pub fn visible_to(mut self, grants: &[String]) -> Self {
        self.packages
            .retain(|package| package.is_public() || grants.contains(&package.directory));
        self
    }

    pub fn for_game(mut self, game: &str) -> Self {
        self.packages.retain(|package| package.game == game);
        self
    }

    pub fn public_only(self) -> Self {
        self.visible_to(&[])
    }

    pub fn get(&self, reference: &crate::PackageRef) -> Option<&crate::PackageInfo> {
        self.packages.iter().find(|package| {
            package.directory == reference.directory && package.name == reference.name
        })
    }
}

#[cfg(test)]
mod tests {
    fn sample() -> super::Manifest {
        super::Manifest {
            packages: vec![
                crate::fixtures::package(crate::PUBLIC_DIR, "FilipekMod_numeric_standard"),
                crate::fixtures::package("premium-voices", "FilipekMod_premium_pack"),
                crate::fixtures::package("beta", "FilipekMod_beta_pack"),
            ],
        }
    }

    #[test]
    fn json_round_trip() {
        let json = serde_json::to_string(&sample()).unwrap();
        let back: super::Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.packages.len(), 3);
        assert_eq!(back.packages[1].directory, "premium-voices");
    }

    #[test]
    fn an_older_manifest_still_deserializes() {
        let old = r#"{"packages":[{"directory":"public","name":"basic",
            "version":"1.0.0","size":5,"sha256":"ab","updated_at":1,
            "category":"pacenotes","tree_path":["ignored"]}]}"#;
        let manifest: super::Manifest = serde_json::from_str(old).unwrap();
        assert_eq!(manifest.packages[0].game, crate::GAME_RBR);
    }

    #[test]
    fn get_finds_by_ref() {
        let manifest = sample();
        let reference = crate::PackageRef::parse("beta/FilipekMod_beta_pack").unwrap();
        assert_eq!(
            manifest.get(&reference).unwrap().name,
            "FilipekMod_beta_pack"
        );
        let missing = crate::PackageRef::parse("beta/nope").unwrap();
        assert!(manifest.get(&missing).is_none());
    }

    #[test]
    fn for_game_filters_one_tab_worth() {
        let mut manifest = sample();
        manifest.packages[1].game = crate::GAME_BEAMNG.into();
        assert_eq!(manifest.clone().for_game(crate::GAME_RBR).packages.len(), 2);
        assert_eq!(
            manifest.clone().for_game(crate::GAME_BEAMNG).packages.len(),
            1
        );
        assert!(manifest.for_game("nosuchgame").packages.is_empty());
    }

    #[test]
    fn public_only_keeps_public_dir() {
        let manifest = sample().public_only();
        assert_eq!(manifest.packages.len(), 1);
        assert!(manifest.packages[0].is_public());
    }

    #[test]
    fn visible_to_unions_grants_with_public() {
        let manifest = sample().visible_to(&["beta".to_string()]);
        let directories: Vec<&str> = manifest
            .packages
            .iter()
            .map(|package| package.directory.as_str())
            .collect();
        assert_eq!(directories, vec![crate::PUBLIC_DIR, "beta"]);
    }

    #[test]
    fn multiple_grants() {
        let manifest = sample().visible_to(&["beta".to_string(), "premium-voices".to_string()]);
        assert_eq!(manifest.packages.len(), 3);
    }
}
