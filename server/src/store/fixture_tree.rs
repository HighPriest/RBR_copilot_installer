pub fn write(path: &std::path::Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// `<data>/<game>/packages/<rest>`.
pub fn package_path(data_dir: &std::path::Path, game: &str, rest: &str) -> std::path::PathBuf {
    data_dir
        .join(game)
        .join(crate::store::PACKAGES_DIR)
        .join(rest)
}

pub fn generate(data_dir: &std::path::Path) -> crate::store::Generated {
    crate::store::generate(data_dir).unwrap()
}

/// A manifest from before generation became pure: `rbr/public/basic` carrying
/// values that exist in no sidecar.
pub fn legacy_manifest() -> pacenotes_shared::Manifest {
    pacenotes_shared::Manifest {
        packages: vec![pacenotes_shared::PackageInfo {
            directory: "public".into(),
            name: "basic".into(),
            version: "2.3.4".into(),
            size: 0,
            sha256: "stale".into(),
            updated_at: 1,
            game: pacenotes_shared::GAME_RBR.into(),
            display_name: Some("Numeric".into()),
            description: Some("Numeric pacenotes".into()),
            exclusive_group: Some("pacenote-config".into()),
            requires: vec!["public/language/polish".into()],
        }],
    }
}
