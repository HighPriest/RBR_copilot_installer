pub fn package(directory: &str, name: &str) -> crate::PackageInfo {
    crate::PackageInfo {
        directory: directory.into(),
        name: name.into(),
        version: "1.0.0".into(),
        size: 123,
        sha256: "ab".repeat(32),
        updated_at: 1_700_000_000,
        game: crate::default_game(),
        display_name: None,
        description: None,
        exclusive_group: None,
        requires: Vec::new(),
    }
}
