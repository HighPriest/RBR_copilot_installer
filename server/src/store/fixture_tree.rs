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
