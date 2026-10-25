/// The body of `GET /packages`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct Manifest {
    pub packages: Vec<crate::PackageInfo>,
    /// By game id, one for every game published, since a game without a
    /// schema publishes nothing; absent from manifests written before 2026-10-25.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub schemas: std::collections::BTreeMap<String, crate::GameSchema>,
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
        self.schemas.retain(|id, _| id == game);
        self
    }

    pub fn public_only(self) -> Self {
        self.visible_to(&[])
    }

    /// Within one game, since every game may publish the same `directory/name`.
    pub fn get(&self, game: &str, reference: &crate::PackageRef) -> Option<&crate::PackageInfo> {
        self.packages.iter().find(|package| {
            package.game == game
                && package.directory == reference.directory
                && package.name == reference.name
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
            schemas: std::collections::BTreeMap::new(),
        }
    }

    fn schema(sha256: &str) -> crate::GameSchema {
        crate::GameSchema {
            sha256: sha256.into(),
            categories: vec![crate::SchemaCategory {
                id: crate::CATEGORY_CODRIVER.into(),
                select: crate::SELECT_ONE.into(),
                install_root: crate::INSTALL_ROOT_GAME.into(),
            }],
        }
    }

    #[test]
    fn schemas_round_trip_and_follow_the_game_filter() {
        let mut manifest = sample();
        manifest
            .schemas
            .insert(crate::GAME_RBR.into(), schema("aa"));
        manifest
            .schemas
            .insert(crate::GAME_BEAMNG.into(), schema("bb"));

        let back: super::Manifest =
            serde_json::from_str(&serde_json::to_string(&manifest).unwrap()).unwrap();
        assert_eq!(back.schemas, manifest.schemas);

        let rbr = back.for_game(crate::GAME_RBR);
        assert_eq!(rbr.schemas.len(), 1);
        assert_eq!(rbr.schemas[crate::GAME_RBR].sha256, "aa");
    }

    #[test]
    fn a_manifest_without_schemas_still_parses_and_stays_lean() {
        let old = r#"{"packages":[]}"#;
        let manifest: super::Manifest = serde_json::from_str(old).unwrap();
        assert!(manifest.schemas.is_empty());
        assert_eq!(serde_json::to_string(&manifest).unwrap(), old);
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
            manifest.get(crate::GAME_RBR, &reference).unwrap().name,
            "FilipekMod_beta_pack"
        );
        let missing = crate::PackageRef::parse("beta/nope").unwrap();
        assert!(manifest.get(crate::GAME_RBR, &missing).is_none());
    }

    #[test]
    fn get_does_not_cross_games() {
        let mut beamng = crate::fixtures::package(crate::PUBLIC_DIR, "language/polish");
        beamng.game = crate::GAME_BEAMNG.to_string();
        beamng.version = "beamng-version".into();
        let mut rbr = crate::fixtures::package(crate::PUBLIC_DIR, "language/polish");
        rbr.version = "rbr-version".into();
        let manifest = super::Manifest {
            packages: vec![beamng, rbr],
            schemas: std::collections::BTreeMap::new(),
        };

        let reference = crate::PackageRef::parse("public/language/polish").unwrap();
        assert_eq!(
            manifest.get(crate::GAME_RBR, &reference).unwrap().version,
            "rbr-version"
        );
        assert_eq!(
            manifest
                .get(crate::GAME_BEAMNG, &reference)
                .unwrap()
                .version,
            "beamng-version"
        );
        assert!(manifest.get(crate::GAME_ASSETTOCORSA, &reference).is_none());
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
