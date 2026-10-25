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

/// Without the bundled-schema comparison, which every fixture schema would trip.
pub fn generate(data_dir: &std::path::Path) -> crate::store::Generated {
    crate::store::generate_with(data_dir, &[]).unwrap()
}

/// Writes `<data>/<game>/packages/<rest>`, adding a zip's category to the
/// game's schema so the fixture publishes.
pub fn write_package(data_dir: &std::path::Path, game: &str, rest: &str, bytes: &[u8]) {
    write(&package_path(data_dir, game, rest), bytes);
    let Some((_, name)) = rest
        .strip_suffix(".zip")
        .and_then(|path| path.split_once('/'))
    else {
        return;
    };
    let category = name.split('/').next().unwrap_or(name);
    let schema = data_dir.join(game).join(pacenotes_shared::SCHEMA_FILE);
    let mut text = std::fs::read_to_string(&schema).unwrap_or_default();
    let entry = schema_entry(category);
    if !text.contains(&entry) {
        text.push_str(&entry);
        write(&schema, text.as_bytes());
    }
}

/// Writes `<data>/<game>/schema.toml` declaring `categories` as optional extras.
pub fn declare(data_dir: &std::path::Path, game: &str, categories: &[&str]) {
    let text: String = categories.iter().map(|id| schema_entry(id)).collect();
    write(
        &data_dir.join(game).join(pacenotes_shared::SCHEMA_FILE),
        text.as_bytes(),
    );
}

fn schema_entry(category: &str) -> String {
    format!("[[category]]\nid = \"{category}\"\nselect = \"any\"\ninstall_root = \"game\"\n")
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
        schemas: std::collections::BTreeMap::new(),
    }
}
